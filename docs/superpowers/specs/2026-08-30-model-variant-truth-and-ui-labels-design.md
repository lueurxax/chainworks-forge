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
- Route the two non-run Steward ACP lanes through the same durable prompt
  authority without inventing synthetic runs, stages, or agent executions.
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

The active-catalog load used by `BackgroundStewardAgentExecutor` delegates to
the same fresh-intake validator instead of deserializing an unchecked catalog.
The current `system_steward` and `steward_auditor` bindings remain Claude and
therefore have no Codex accepted-pair claim. If a later catalog binds either
agent to one of the approved Codex profiles, that invocation carries
`codex_exact_pair_v1` and must complete the owner-scoped negotiation and receipt
flow below. A raw active-catalog load cannot bypass fresh validation.

The author-authored YAML does not contain a compatibility switch. After
validation, the compiler writes a compiler-owned marker into the immutable
catalog snapshot:

```json
{
  "chainworks_compiled": {
    "schema_version": 2,
    "provider_configuration_policy_version": "provider_configuration_policy_v1",
    "backend_profile_capabilities": {
      "codex_builder_high": "codex_exact_pair_v1",
      "claude_builder_high": "not_applicable_v1"
    }
  }
}
```

This is an abbreviated excerpt; the existing embedded-skill fields remain in
the same object. Only `chainworks_compiled.schema_version` advances from 1 to
2; the outer `catalog_snapshot_format_version` remains 2. The complete map has
one entry for every frozen backend profile. A Codex profile in the approved
matrix is `codex_exact_pair_v1`; every non-Codex profile is
`not_applicable_v1`. The frozen catalog bytes and each `ResolvedAgent` carry its
immutable `snapshot_provider_capability`.

### Frozen snapshot replay

`compile_from_snapshot_json()` reads only compiler-owned frozen capability:

- `codex_exact_pair_v1` requires the frozen pair to be structurally complete
  and uses required negotiation;
- `not_applicable_v1` requires a non-Codex provider and performs no Codex
  accepted-pair operation; and
- a pre-change snapshot with schema v1 or no marker is
  `legacy_best_effort_v0`.

The replay path never re-applies the current seven-profile matrix. It does not
infer the contract from the current catalog, a model name, or application
defaults. Pre-change frozen snapshots therefore retain the old adapter
behavior: no required model operation and the prior best-effort effort
operation. Their snapshot bytes are not upgraded in place.

Runtime fallback never reuses the source binding's capability. Before enqueue,
`EffectiveProviderContractV1` is derived from the selected target profile in
the same frozen catalog. It has exact keys `schema_version`, source and target
backend-profile IDs, fallback reason, provider, requested model/effort,
`snapshot_provider_capability`, and target binding digest. Its RFC 8785 digest
is persisted in the InvokeAgent envelope, work item, `ExecutionRequest`, and
`AgentExecution`; permit CAS verifies all four copies. Codex-to-non-Codex
fallback therefore records `not_applicable_v1` and cannot inherit accepted
Codex truth. Non-Codex-to-Codex fallback records `codex_exact_pair_v1` and must
complete exact negotiation. `legacy_best_effort_v0` remains only for a frozen
legacy target profile. Fallback changes effective contract and attempt binding,
but not the original compiled-task or occurrence identity.

## Stable Task-Occurrence Identity

Ownership is split explicitly across crates:

- `domain::invoke_agent_contract` owns the sealed `ProducerIdV1` enum,
  canonical byte codec, `InvokeAgentEnvelopeV1`, and an opaque
  `ValidatedInvokeAgentEnvelopeV1`. Envelope fields are private. The only way
  to obtain the validated type is `compile()` or `parse_and_validate()`, both of
  which recompute every identity and reject unknown producer IDs or extra keys.
- `engine::invoke_agent_dispatch::InvocationOccurrenceFactory` maps typed
  producer context into the domain compiler. Production producers receive no
  API that accepts a caller-supplied compiled-task or occurrence ID.
- `db::repos::work_items::enqueue_invoke_agent[_tx]` accepts only the opaque
  validated type and owns the SQLite insert. Generic `enqueue[_tx]` rejects
  `WorkItemKind::InvokeAgent`; raw InvokeAgent SQL and payload mutation are
  private to this repository module. Claim parses and revalidates the envelope
  before changing queue state.

This boundary is structural rather than an inventory-only convention. A
recursive gate rejects direct InvokeAgent SQL, generic enqueue, public raw
constructors, or payload mutation outside the dedicated domain/db modules.

`InvokeAgentEnvelopeV1` requires run ID, owner kind/ID, nullable stage execution
ID, compiled-task ID, task-occurrence ID, a sealed `InvokeSourceProvenanceV1`,
captured run dispatch epoch, per-attempt effective provider contract, and the
existing provider, agent, session-reuse, and payload fields. The factory derives
identity before the queue row becomes visible; the claim path recomputes and
validates the tuple against durable owner truth before creating or reusing an
`AgentExecution`.

`CompiledTaskCoordinateV1` is compiler-owned frozen data, not an orchestrator
guess. Static/owner/mediation coordinates are complete in RunPlan; a dynamic
coordinate is completed by `InvocationOccurrenceFactory` from the compiler's
frozen dynamic template plus a validated selector row before either durable
materialization or work-item insert. It has exactly `workflow_snapshot_sha256`, `state_id`, `container_kind`,
`block_kind`, `block_ordinal`, `lane_ordinal`, `task_ordinal`, `task_name`,
nullable `dynamic_task_key`, and `frozen_binding_sha256`. `container_kind` is
`run`, `run_after_approval`, `owner`, `mediation`, or `legacy_flat` and therefore
prevents the same state/task text on the two current `RunBlock` fields from
colliding. Current DSL block ordinals are fixed independently of field presence:
`sequence = 0`, `parallel = 1`, `then = 2`, `dynamic_parallel = 3`, and
`owner = 4`. For sequence/then, `lane_ordinal = 0` and `task_ordinal` is YAML
array position. For flat parallel, `lane_ordinal` is YAML array position and
`task_ordinal = 0`. Dynamic selection uses selected-agent array position as its
lane and zero as its task ordinal. Owner, mediation, and legacy coordinates use
their explicit component tables below. All ordinals are frozen by the compiler
before a stage execution exists.

`frozen_binding_sha256` is SHA-256 over duplicate-key-rejected RFC 8785 bytes of
`FrozenInvocationBindingV1`. Its exact required keys are `schema_version`,
`agent_id`, nullable `backend_profile_id`, `provider`, nullable `model`, nullable
`effort`, nullable `max_turns`, nullable `temperature`, `prompt_sha256`, nullable
`permission_profile`, nullable `skill_ref`, nullable `skill_role`, nullable
`skill_snapshot_hash`, sorted `requested_mcp_server_ids`, nullable
`output_contract`, `worktree_write_enabled`, nullable `worktree_strategy`,
nullable `session_reuse_scope`, nullable `session_family_id`,
`xcode_broker_required`, `xcode_shim_injection_signal`,
`requires_xcode_host_execution`, nullable `toolchain_cache_policy_sha256`, and
the task's ordered `inputs`, ordered `outputs`, canonical `output_policies`, and
canonical `output_schema_sha256s`. `schema_version` is
`frozen_invocation_binding_v1`; prompt and nested schema/cache objects are
represented only by their lowercase 64-hex digests. This replaces the existing
map-order-sensitive runtime fingerprint as compiled identity input.

`InvokeSourceProvenanceV1` is a required tagged union:

- `production.compiled_snapshot` carries `producer_id`, the full
  `CompiledTaskCoordinateV1`, its canonical digest, and the frozen RunPlan
  snapshot digest;
- `production.dynamic_materialization` additionally carries the canonical
  selector artifact digest, frozen selection `plan_hash`, binding ID, selected
  agent ID, selection ordinal, and `dynamic_task_key`;
- `production.preserved_envelope` carries `producer_id`, source work-item ID,
  source envelope SHA-256, and the source compiled-task ID; and
- `legacy_migration` carries upgrade ID, legacy work-item ID, untouched payload
  SHA-256, durable owner tuple, and workflow identity marker.

Production compilation may construct only a `production.*` variant. Claim
loads the frozen RunPlan/dynamic/source work-item rows, recomputes the provenance
digest and every identity, and rejects a missing or changed source before owner
mutation. The upgrade module may construct only `legacy_migration`; its DB write
API is private to `ProviderTruthUpgradeCoordinator`. There is no free-standing
`migration_source` field that can escape this union.

Every queue row stores `invoke_envelope_sha256`, computed over duplicate-key-
rejected RFC 8785 envelope JSON with that adjacent column absent. A preserved
producer must reference the exact prior row ID and digest; claim verifies both
rows and the frozen provenance object in one transaction. Unknown union tags,
missing source rows, digest mismatch, or a production row carrying migration
provenance fail before status mutation.

The exact production `ProducerIdV1` vocabulary is frozen to the nine IDs in the
current producer manifest. `ProducerIdV1` records which production route
created or preserved an envelope; it is not itself a component of logical task
identity. `legacy_migrated` is deliberately absent from this enum and is
represented only by `LegacyInvokeEnvelopeV1` during upgrade.

The compiled coordinate modes and exact ordered component arrays are:

| Producer ID | Mode | Ordered `compiled_task_v1` components |
|---|---|---|
| `orchestrator.standard_task` | compile | `[workflow_snapshot_sha256, state_id, container_kind, block_kind, block_ordinal, lane_ordinal, task_ordinal, task_name, frozen_binding_sha256]` |
| `orchestrator.dynamic_parallel` | compile | `[workflow_snapshot_sha256, state_id, "run", "run.dynamic_parallel", "3", lane_ordinal, "0", task_name, dynamic_task_key, frozen_binding_sha256]` |
| `orchestrator.legacy_flat` | compile | `[workflow_identity_marker, stage_id, "legacy_flat", "legacy_flat", "0", "0", task_ordinal, task_name, frozen_binding_sha256]` |
| `orchestrator.owner_only` | compile | `[workflow_snapshot_sha256, state_id, "owner", "owner", "4", "0", "0", owner_agent_id, frozen_binding_sha256]` |
| `orchestrator.p017_mediation` | compile | `[workflow_snapshot_sha256, origin_state_id, "mediation", "lead_conflict_mediation", "0", "0", "0", mediation_task_kind, frozen_lead_binding_sha256]` |
| `command_handler.targeted_retry` | preserve | Exact validated source `compiled_task_id`; recompute only occurrence when owner changes |
| `orchestrator.auto_contract_retry` | preserve | Exact validated source `compiled_task_id` and same-owner occurrence |
| `orchestrator.p058_escalation_retry` | preserve | Exact validated source `compiled_task_id` and same-owner occurrence |
| `p058_deadline_resume.operator_resume` | preserve | Exact validated source `compiled_task_id`; occurrence uses the linked replacement owner when one is created |

`block_kind` is one of `run.sequence`, `run.parallel`, `run.then`,
`run.dynamic_parallel`, `owner`, `lead_conflict_mediation`, or `legacy_flat`.
The ordinal rules are those of `CompiledTaskCoordinateV1` above and all integer
components use canonical base-10 strings. `dynamic_task_key` is
`dynamic_task_v1:<sha256>` over common-codec components
`[selector_artifact_sha256, plan_hash, materialization_binding_id, selected_agent_id, selection_ordinal]`;
the selector digest is over canonical
decoded `AgentSelectionPlanV1`, not filesystem bytes. The selection plan must
carry the same binding ID and agent at that ordinal. Materialization row ID,
work-item ID, stage-execution ID, loop counter, selected provider, and runtime
fallback binding are forbidden components. `workflow_identity_marker` is the frozen snapshot SHA-256 when
available and otherwise the persisted string
`legacy_workflow_v0:<workflow_id>`; it is never recomputed from current YAML.
`frozen_binding_sha256` is the exact `FrozenInvocationBindingV1` digest above.
Provider fallback may change the runtime binding but cannot change this value.
Legacy coordinates freeze container/block `legacy_flat`, zero block/lane, and
the migrated task ordinal/name. Mediation freezes container `mediation`, block
`lead_conflict_mediation`, zero ordinals, and task name equal to the closed
mediation task kind. Owner task name equals the frozen owner agent ID. Preserve
producers carry the source coordinate and its digest unchanged.

Provider fallback is a binding change on one of those producer-owned
invocations, not a tenth raw producer. Same-owner retry/fallback preserves the
compiled-task and occurrence IDs. A targeted retry with a new stage execution
and every loop re-entry preserve the compiled-task ID but recompute occurrence
from the new owner. The enum-generated manifest must byte-match the checked-in
inventory, so adding a tenth variant fails the gate until its identity and
behavior fixture are added.

Every source first receives `compiled_task_v1:<sha256>`. The hash input is
`UTF8("chainworks.compiled_task.v1") || 0x00`, followed by each normalized
component in the producer-specific order above as a u32 big-endian byte length
followed by UTF-8 bytes. UUIDs are lowercase hyphenated, ordinals are canonical base-10
without leading zeroes, hashes are lowercase hex, and no Unicode normalization
or locale folding is performed. `task_occurrence_v1:<sha256>` uses the same
codec with domain `chainworks.task_occurrence.v1` and ordered components
`owner_kind`, `owner_id`, `compiled_task_id`.

Golden vectors are normative. For `orchestrator.standard_task`, the complete
component array is:

```json
[
  "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
  "state_2",
  "run",
  "run.sequence",
  "0",
  "0",
  "0",
  "draft",
  "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
]
```

The compiled digest is
`2d4aa12abb242e8f234f79e317fca2c8cfc62c9d693af49a821dfed5c021639f`.
The occurrence component array is exactly
`["stage_execution", "11111111-1111-4111-8111-111111111111", "compiled_task_v1:2d4aa12abb242e8f234f79e317fca2c8cfc62c9d693af49a821dfed5c021639f"]`;
its digest is
`134bccbb17bc96a750d3f570c389deb56463ca91b04dce6dfe94379f4c5adc1f`.
The gate reproduces both with the Rust implementation and an independent
checked-in Python reference encoder. Fixtures include every producer, the
complete component arrays, reordered input, empty component, non-ASCII byte,
malformed length, unknown producer, `run` versus `run_after_approval`, every
sequence/parallel/then lane position, owner/mediation/legacy coordinates,
dynamic selector replans with same agent/different plan or binding, and
forbidden dynamic-component cases. A pairwise collision harness requires all
logically distinct coordinates to produce distinct IDs.

The dynamic migration rebuilds `dynamic_materialization_records` with
`stage_execution_id`, `compiled_task_id`, `task_occurrence_id`, unique
`work_item_id REFERENCES work_items(id)`, and nullable true
`agent_execution_id REFERENCES agent_executions(id)`. The old column currently
named `agent_execution_id` contains a work-item ID; migration copies it only to
`work_item_id`, sets the real execution link null, and derives identity for
provably pending rows through the legacy-upgrade path below. New materialization
uses one `db::repos::dynamic_materialization::insert_with_invoke_tx` transaction
to insert both rows. On idempotency conflict it loads the winner and requires
all immutable identity bytes to match; mismatch blocks the stage, and match
returns the existing work item without a second enqueue. Claim fills the true
agent-execution link in its execution-creation transaction. Topology reads
these durable fields, never agent name or selection order.

The occurrence ID is copied into the work-queue payload, `ExecutionRequest`,
and `AgentExecution`. Two tasks in one stage that use the same agent receive
different IDs. A later loop iteration or replacement stage execution receives a
new ID, so attempts from different owner scopes cannot be merged. Pending
static/owner topology exposes `compiled_task_id`; its
`task_occurrence_id` remains `null` until a stage execution exists.

One domain projection, `RunStageTopologyOccurrenceV2`, represents every row.
Its `source_kind` is closed to `static_compiled`, `owner_compiled`,
`dynamic_materialized`, or `legacy_flat`; it also carries a non-null
`source_stable_id`, `compiled_task_id`, frozen stage/task ordinals, planned
binding, nullable occurrence/execution truth, and association state. Static and
owner source IDs are compiler-owned IDs in the RunPlan snapshot. Dynamic source
ID is the durable materialization ID created before its work item becomes
visible. Migration creates `legacy_topology_occurrence_v1:<sha256>` over
`[run_id, workflow_identity_marker, state_id, task_ordinal, agent_id]`; it does
not require a StageExecution. Consequently every planned, pending, dynamic, and
legacy row has a stable presentation source before its first execution.

The compiler persists `frozen_workflow_ordinal` on every topology node and
`frozen_task_ordinal` on every occurrence. State ordinal is source YAML state
sequence order after the parser's preserved mapping order; the compiler fails
if that order is unavailable rather than deriving it from a hash map. Migration
uses the persisted historical stage order when unique and otherwise orders by
`stage_id` while marking `legacy_order_unverified`; the UI surfaces that state
and never calls it frozen truth.

Topology association follows these rules:

1. Match executions to occurrences by `task_occurrence_id`.
2. Count attempts only within that occurrence.
3. Select the latest execution within that occurrence for requested/accepted
   readback.
4. For pre-change executions whose occurrence ID is `null`, allow the old
   `agent_id` fallback only when exactly one task in that stage uses that agent.
5. If a legacy stage is ambiguous, do not assign an execution to either task
   and do not guess runtime truth.

The gate combines the structural scan with behavior tests for all nine exact
producer IDs, same-owner retry/fallback, replacement-stage retry, loop re-entry,
dynamic idempotency replay/conflict, and `legacy_flat` migration.

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
   `ProviderConfigurationAcceptanceV1` and owner-scoped
   `ProviderConfigurationReceiptV1`; for a run agent, project the same receipt
   onto its `AgentExecution` in that transaction.
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

When they match, one transaction derives a new owner-bound
`ProviderConfigurationReceiptV1` from the generation acceptance and writes it
to the authoritative receipt table. For a run agent it also writes the exact
receipt projection to the new `AgentExecution` with
`acceptance_source = reused_session_generation` and the source acceptance
digest. The derived receipt names the new agent execution and task occurrence;
it does not copy the first execution's IDs. That projection is
response-verified authority inherited from the same live provider session; it
is not a new negotiation.

When evidence is absent, stale, malformed, or mismatched, the manager closes
and invalidates the generation before any prompt. For an original InvokeAgent
or a Steward lane still before its first prompt, it may then perform at most one
fresh-session fallback through the complete negotiation transaction. P079 and
P086 owners fail closed instead because their contracts require the parent or
attached generation. The old session receives zero prompts. An allowed
fresh-session failure is returned normally and is not retried again by this
compatibility path.

Legacy v0 generations may be reused only by a
`legacy_best_effort_v0` execution. A `codex_exact_pair_v1` request never
inherits legacy-unverified generation evidence.

## Durable Runtime Truth

The next SQLite migration adds these columns to `agent_executions`:

| Column | Meaning |
|---|---|
| `task_occurrence_id` | Stable occurrence shared only within one owner scope |
| `requested_model` / `requested_effort` | Canonical pair requested for this execution |
| `accepted_model` / `accepted_effort` | Canonical response-verified pair; otherwise `null` |
| `accepted_model_wire_value` / `accepted_effort_wire_value` | Exact provider option values whose `currentValue` was verified |
| `provider_configuration_state` | `configuring`, `configured`, `failed_before_prompt`, `cancelled_before_prompt`, or `legacy_unverified`; `null` for non-Codex |
| `provider_configuration_verified_at` | Complete-pair verification time; otherwise `null` |
| `provider_configuration_receipt_json` / `provider_configuration_receipt_sha256` | Bounded projection of the authoritative owner-scoped receipt and its verified digest |
| `acceptance_source` | `fresh_negotiation` or `reused_session_generation`; otherwise `null` |
| `configuration_evidence_state` | Non-null `pending`, `receipt_available`, `receipt_unavailable`, `not_applicable`, or `legacy_unverified` |
| `next_configuration_attempt_index` | Non-null monotonic allocator, initialized to `0` |
| `current_provider_configuration_receipt_id` | Nullable FK to the latest successfully persisted owner receipt |
| `snapshot_provider_capability` | Immutable capability from the original frozen binding |
| `effective_provider_contract_json` / `effective_provider_contract_sha256` | Per-attempt provider/model/effort contract after fallback selection |

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
- rebuilds `session_lineages` with the owner contract below and extends
  `session_generations` with contract version, canonical and wire accepted
  pairs, provider-session binding fingerprint, acceptance JSON/digest, and
  verified-at fields;
- creates durable `steward_agent_lanes` before any Steward provider call and
  rebuilds `provider_sessions` plus new `provider_process_bindings` so both run
  and non-run generations have typed process ownership;
- creates `provider_configuration_receipts`, the owner-scoped accepted-pair
  authority described below; `agent_executions` stores only its lockstep
  projection;
- creates `provider_configuration_failures` for zero-send attempts whose
  authoritative receipt could not be committed;
- rebuilds `agent_external_side_effect_ledger` with provider-send states and a
  prompt-turn FK, rather than writing P086 v2 values into the old v065 CHECK;
- adds `next_prompt_turn_index INTEGER NOT NULL DEFAULT 1` to
  `agent_executions`, backfilled to one greater than the greatest migrated turn;
- creates `provider_prompt_turns` as the sole dispatch authority; and
- rebuilds `agent_execution_runtime_receipts` with the explicit legacy-link
  states below. It remains terminal receipt evidence and retains non-null
  `receipt_json`; it never represents a pre-send row.

`steward_agent_lanes` has deterministic primary key
`steward_lane_v1:<sha256>` over exact components `[analysis_id, agent_id]`,
non-null analysis ID, agent ID, claimed StewardAnalysis work-item ID, provider,
requested model/effort, contract version, lane state, and timestamps; nullable
lineage, generation, configuration-receipt, prompt-turn, terminal artifact, and
typed failure fields follow lifecycle state. It also has
`next_configuration_attempt_index INTEGER NOT NULL DEFAULT 0`, nullable
`current_provider_configuration_receipt_id` FK, and non-null
`configuration_evidence_state` under the same rules as an execution. The agent check is
`system_steward | steward_auditor`; `(analysis_id, agent_id)` is unique. Lane
state is closed to `reserved`, `configuring`, `configured`, `prompt_pending`,
`prompt_sent`, `completed`, `configuration_failed`, `prerequisite_skipped`,
`cancelled_before_prompt`, `failed`, `prompt_delivery_unknown`, and
`legacy_unverified`. The last seven values are terminal.

At claim, `run_steward_analysis_with_executor` computes all deterministic
analysis inputs and, in one transaction, inserts the `steward_analyses` row as
`running`, inserts both lane rows as `reserved`, and binds the already claimed
StewardAnalysis work item. Provider calls happen only afterward. Final
settlement updates that analysis row instead of inserting it late. Therefore
`analysis_id` and lane ownership survive a crash before either provider call.

Both lane rows always settle. `system_steward` runs first. If no executor is
available, both lanes become `prerequisite_skipped(agent_executor_unavailable)`.
If system configuration or dispatch fails before a health report can exist, its
lane becomes `configuration_failed`, `failed`, or `prompt_delivery_unknown` as
appropriate and the auditor becomes
`prerequisite_skipped(system_health_report_unavailable)`. If system completes
without the required health report, it is `failed(missing_health_report)` and
the auditor is skipped for the same prerequisite reason. Only a validated
health report permits the auditor to leave `reserved`; its own pre-prompt
configuration failure is `configuration_failed`. After both lanes are terminal,
the analysis and single StewardAnalysis work item settle atomically as
`completed`, `completed_with_lane_failures`, or `failed` according to the frozen
two-lane matrix. No terminal or skipped lane is eligible for startup requeue.

`session_lineages` gains non-null `lineage_owner_kind` and
`lineage_owner_id` plus nullable `steward_lane_id` FK. Kind is `run_agent` or `steward_agent_lane`. For
`run_agent`, `run_id` is non-null and equals `lineage_owner_id`; for Steward,
`run_id` is null, `steward_lane_id` is non-null and equals
`lineage_owner_id`, and `agent_id` equals that lane's agent. Existing rows
backfill `run_agent` with null lane FK.
`provider_sessions` similarly gains owner kind `agent_execution` or
`steward_agent_lane`, owner ID, nullable `run_id`, and nullable
`steward_lane_id` FK; its CHECK requires the matching execution/run tuple or
the matching non-run lane tuple. No synthetic RunId or AgentExecution is
permitted.

`provider_process_bindings` is keyed by session-generation ID and contains the
typed owner tuple (`agent_execution`, `p086_continuation`, or
`steward_agent_lane`), the matching nullable owner FK, provider, child PID,
process-group ID where supported,
process-start identity, daemon-generation ID, and state `launching`, `running`,
`spawn_pending`, `exit_observed`, `reaped`, or `identity_ambiguous`. Before
spawn, the supervised launcher inserts `spawn_pending` with a random launch
nonce. On macOS it starts the provider behind a runtime-owned one-shot launch
barrier; the child cannot execute provider code until PID/start identity are
persisted as `launching` and the parent releases the barrier. EOF before release
terminates the child. The launcher must persist `launching` before writing
`session/new`; a provider session later references this row. An empty or
unverifiable process-start identity is `identity_ambiguous`, closes prompt
admission, and is never signalled by PID alone.

`provider_prompt_quarantines` stores evidence even when the authoritative turn
does not exist. It has primary key, non-null owner kind/ID, nullable run,
stage-execution, agent-execution, work-item, and existing prompt-turn FKs,
nullable `missing_prompt_turn_id` as diagnostic text, closed reason, bounded
evidence JSON/digest, state `active | settled`, and timestamps. Owner kinds are
`invoke_agent_work_item`, `p079_lease`, `p086_continuation`,
`steward_work_item`, and `steward_agent_lane`; reason includes
`missing_authority`, `legacy_owner_ambiguous`, `delivery_unknown`, and
`process_identity_ambiguous`. A partial unique index permits one active row per
owner. Missing-authority settlement inserts this row and terminalizes or holds
the owning domain row in one transaction; it never requires a foreign key to a
row whose absence is the evidence. Startup and every replay selector reject an
active quarantine.

`provider_configuration_receipts` has primary key `id`, non-null
`configuration_owner_kind`, `configuration_owner_id`, non-negative
`configuration_attempt_index`, `work_item_id`, provider, requested pair,
configuration state, bounded receipt JSON/digest, and
created/updated timestamps; execution, occurrence, generation/session,
nullable `steward_lane_id` FK, accepted pair, wire pair, source digest,
verified time, and failure code follow
the nullability rules of `ProviderConfigurationReceiptV1`. Owner kind is
`agent_execution` or `steward_agent_lane`. A database CHECK requires both
execution and occurrence for `agent_execution`, requires the owner ID to equal
the execution ID with null lane FK, and requires both fields null plus a
non-null lane FK equal to the owner ID for `steward_agent_lane`.
`(configuration_owner_kind, configuration_owner_id, configuration_attempt_index)`
is unique. The owner row stores the next attempt
index and current receipt ID. A permitted zero-send renegotiation appends a new
attempt and atomically moves that pointer; it never overwrites prior evidence.
A configured run-agent insert writes the receipt, pointer, and exact
`agent_executions` projection in one transaction; mismatch on read is evidence
corruption. A configured Codex Steward invocation writes the receipt and lane
pointer because no synthetic `AgentExecution` exists.

Allocation is one owner-row CAS inside `BEGIN IMMEDIATE`: read
`next_configuration_attempt_index = n`, update it to `n + 1` with predicate
`next_configuration_attempt_index = n`, and return `n`; a lost CAS retries from
the new value. Receipt settlement
inserts `(owner, n)` and moves `current_provider_configuration_receipt_id` only
when the owner is still on attempt `n`. Gaps left by a crash are valid and are
never reused. Historical rows backfill next index `0` and null pointer; rows for
which this migration creates a receipt backfill next index to one greater than
the greatest inserted attempt and point at that exact row. The final schema
rebuild installs both owner-pointer FKs after the receipt table exists. Tests run
two concurrent allocators for each owner kind and prove unique indexes, monotonic
gaps, pointer CAS, and stale-attempt rejection.

`provider_configuration_failures` is append-only and unique on owner kind/ID
plus attempt index. It stores work item, nullable generation/process binding,
typed failure code, optional source-acceptance digest, cleanup state
`cleanup_pending | reaped | identity_ambiguous`, and timestamps, but no accepted
pair or provider-session secret. If receipt persistence fails after provider
acceptance, the receipt transaction rolls back, the manager sends zero prompt,
and the coordinator closes the generation. A separate minimal settlement
transaction writes this failure row, sets owner configuration to
`failed_before_prompt`, evidence to `receipt_unavailable`, leaves current receipt
null, and keeps the turn `not_started`. If even that settlement cannot commit,
the daemon enters failed-serve; startup finds the still-configuring generation,
identity-checks/reaps it, and writes the same failure before consumers open.
Neither path invents a `ProviderConfigurationReceiptV1`.

`provider_prompt_turns` has `id` as primary key; non-null `prompt_kind`,
`turn_index`, `prompt_owner_kind`, `prompt_owner_id`, `work_item_id`, `provider`,
and `transport_family`; nullable generation/session IDs, agent execution,
occurrence, captured run epoch, and `steward_lane_id` FK; contract version; `dispatch_state`;
start/sent/unknown timestamps; typed failure code; and created/updated
timestamps. Foreign keys bind execution when present and always bind the work
item. Owner kind is `invoke_agent`, `p079_repair`, `p086_continuation`, or
`steward_agent_lane`. A CHECK requires execution, occurrence, and run epoch for
the first three with null lane FK and requires all three null plus a lane FK
equal to the owner ID for Steward. Partial unique indexes
enforce `(agent_execution_id, turn_index)` when an execution exists and
`(prompt_owner_kind, prompt_owner_id, prompt_kind)`. New state is non-null and
checked to `not_started`, `dispatch_pending`, `prompt_sent`, or
`dispatch_unknown`; legacy ambiguity is represented as `dispatch_unknown`, not
SQL null. No receipt JSON or terminal provider status lives in this table.

`PromptTurnAllocator::reserve_tx` is the only constructor. Claim/start inserts
`original/0`, sets `next_prompt_turn_index = 1`, and creates the execution in one
transaction. Every later run-bound prompt atomically reads/increments that
counter, so P079 and P086 cannot both claim index 1. A Steward invocation uses
the durable `steward_agent_lanes.id`, inserts
`steward_analysis/0`, and never allocates from an `AgentExecution`. Exact prompt
kinds are `original`, `code_writer_completion_repair`,
`output_contract_repair`, `work_continuation_live_handle`,
`work_continuation_resurrection`, `work_continuation_output_only`, and
`steward_analysis`; adding a kind requires a migration-safe enum and gate
fixture. A deterministic `prompt_turn_v1:<sha256>` hashes prompt owner kind/ID,
allocated index, kind, a tagged nullable execution/occurrence tuple, and
work-item ID with the canonical length-prefixed codec.

The existing runtime receipt primary key on agent execution, prompt kind, and
turn index remains compatible for run-bound execution receipts. The rebuilt
table adds nullable `prompt_turn_id` and non-null `prompt_link_state`, closed to
`linked_v2`, `legacy_pre_prompt`, or `legacy_unverified`. `linked_v2` requires a
turn foreign key; either legacy state requires it to be null. Every new receipt
write is `linked_v2` and must match the turn tuple. A receipt is terminal
attempt evidence, including a typed failure before send, but is never dispatch
authority. An original, repair, or continuation receipt cannot overwrite
another turn. Steward has no row in this execution-only table: its prompt turn
is dispatch authority, while terminal success/failure remains in the existing
Steward lane/analysis result.

Historical receipt migration is evidence-driven. A unique occurrence/work-item
owner plus positive `handshake.prompt_sent_at_ms` creates or links a
`prompt_sent` turn and marks `linked_v2`. A typed terminal pre-prompt/startup
failure with no positive send timestamp becomes `legacy_pre_prompt`; absent,
null, contradictory, or non-unique owner evidence becomes
`legacy_unverified`. Neither legacy state creates a turn or may be promoted to
`prompt_sent`. Thus old terminal receipts remain representable without an
invented non-null foreign key, while a running owner with unverified evidence
is quarantined and cannot replay.

### P079 lease v2

An append-only migration rebuilds `output_contract_repair_leases` as
`output_contract_repair_leases_v2`. Its state check is `reserved`,
`dispatch_pending`, `prompt_sent`, `dispatch_unknown`, or `settled`; it adds
`repair_prompt_kind`, nullable `work_item_id`, non-null
`work_item_link_state`, nullable `prompt_turn_id`, `dispatch_started_at`,
`prompt_sent_at`, and `dispatch_unknown_at`. `repair_prompt_kind` is
`code_writer_completion_repair` or `output_contract_repair` for a repair lease
and null for a fallback lease. `work_item_link_state` is `linked_v2` or
`legacy_unverified`. For every new repair lease, `linked_v2` requires a non-null
repair work-item FK and its P079-owned prompt-turn FK. For every new fallback
lease, it requires the fallback InvokeAgent work-item FK and that child's
`original` prompt-turn FK; the lease projects but does not own that turn. Only
migrated terminal/unverified rows may have either FK null.
`dispatch_committed_at` remains a deprecated readback alias and equals
`prompt_sent_at` only for v2 rows. Domain enums, repository parsers, indexes,
TTL sweeps, and reference schemas change in the same release.

Reservation atomically consumes exactly the budget selected by lease kind,
binds the current work item, and creates or binds the selected turn in
`not_started`. A repair sets only `repair_budget_consumed`; a fallback sets only
`fallback_budget_consumed`. The opposite flag must remain false. A false budget
flag is illegal on any new active lease. Permit moves lease/turn to
`dispatch_pending` and sets only `dispatch_started_at`; successful flush plus
final CAS moves both to `prompt_sent` and sets `prompt_sent_at`; ambiguous
delivery moves both to `dispatch_unknown`. Terminal output settlement moves the
lease to `settled` without changing canonical turn truth. Budget consumption is
never refunded. A TTL-expired `reserved` lease may settle `deadline_exceeded`
and use only the existing bounded infrastructure retry allowance. Pending,
sent-without-result, or unknown expiry settles with new result
`delivery_unknown`, records `ttl_expired_dispatch_pending` or
`ttl_expired_prompt_sent`, and blocks replay.

The state/nullability/budget matrix is normative:

| Lease row | Work item / turn | Budget truth | Migration/result |
|---|---|---|---|
| New repair, any active state | Repair item and P079 turn non-null | Repair true, fallback false | State mirrors its turn atomically |
| New fallback, any active state | Fallback InvokeAgent item and original turn non-null | Repair false, fallback true | Lease mirrors the child original turn |
| Migrated terminal v1 | Nullable, `legacy_unverified` when not uniquely provable | Preserve already consumed kind; never consume the opposite kind | `settled`, never replayed |
| Active v1 `reserved`, unique owner/kind, budget false | Create/bind one `not_started` turn in the same transaction | Consume selected budget once; if unavailable settle `budget_exhausted` | Remain `reserved` only after both commits |
| Active v1 `reserved`, unique owner/kind, budget true | Create/bind one `not_started` turn | Validate exactly the matching kind flag | Remain `reserved` |
| Active v1 `prompt_sent` or old send-side effect | Bind a turn only with unique owner; otherwise null plus quarantine | Atomically adopt one historical attempt for the matching budget when false | Always `dispatch_unknown`, never proven sent |
| Any active row with ambiguous owner, kind, turn, or contradictory budget flags | Nullable legacy links | Preserve flags; consume nothing new | `dispatch_unknown` plus quarantine |

Migration maps terminal v1 leases to `settled` without inventing prompt truth.
For every active v1 lease, the upgrader queries
`artifact_source_generation_claims` by exact run ID, stage-execution ID,
`agent_execution_id = parent_agent_execution_id`, and an existing source work
item. Exactly one distinct `source_work_item_id` is required. A unique match
sets `linked_v2`. `P079PromptKindClassifierV1` then derives the planned kind
from immutable run-plan/output-contract inputs plus the repair event; an
existing runtime receipt, when present, must agree. Only a unique classification
plus the atomic budget transition above allows an active `reserved` lease to
remain reserved. Missing/contradictory classification, or zero or multiple
work-item matches, sets `legacy_unverified`, moves the lease to
`dispatch_unknown`, inserts an owner quarantine without a prompt turn, and
blocks replay.

An active v1 `prompt_sent` row proves only that the old pre-I/O database write
occurred. Even with a unique work-item match it becomes `dispatch_unknown`,
uses the old `dispatch_committed_at` as `dispatch_started_at`, records migration
time as `dispatch_unknown_at`, creates the matching turn only when all owner
fields are unique, and blocks its stage/run. The upgrader covers both existing
repair prompt kinds and fallback leases; no historical active row is upgraded
to proven `prompt_sent`. Generated fixtures cross lease kind, v1 state, both
budget flags, zero/one/multiple work-item matches, classifier result, and runtime
receipt evidence, and assert the table above byte-for-byte.

### P086 atomic owner reservation

P086 admission pre-generates the continuation and ProcessContinuation work-item
IDs. One `BEGIN IMMEDIATE` transaction performs command idempotency and policy
checks, inserts the command-journal row, continuation, Pending work item,
allocated `not_started` turn, and `provider_send` side-effect row in `reserved`.
The work item stores non-null run/stage owner fields. Any insert failure rolls
back all five records; an accepted response therefore always names a claimable
work item and turn. Identical command replay returns the committed IDs and
cannot enqueue a second item.

The same migration fully rebuilds `agent_external_side_effect_ledger`. It adds
nullable `prompt_turn_id REFERENCES provider_prompt_turns(id)` and replaces the
old status CHECK with a kind-sensitive CHECK. `provider_send` requires a
non-null turn and one of `reserved`, `dispatch_pending`, `prompt_sent`,
`dispatch_unknown`, or `failed`. Every other existing side-effect kind requires
a null turn and retains `planned`, `started`, `committed`, `released`, or
`failed`. A unique constraint on
`(continuation_id, side_effect_kind, sequence_number)` and a partial unique constraint on
`(continuation_id, prompt_turn_id)` prevent a second provider send. Permit,
flush, and ambiguity update this row and the authoritative turn with matching
old/new states in one CAS transaction.

`P086LegacyDispatchClassifierV1` evaluates the complete cartesian product of
continuation mode/status, ProcessContinuation status, old provider-send row
absence or `planned|started|committed|failed`, attach-receipt
absence/failure/success, and terminal runtime-receipt evidence. The reduction is
closed:

| Historical evidence | v2 result |
|---|---|
| No send row, continuation accepted/queued, valid bound work item, no positive I/O evidence | `reserved` with a newly allocated `not_started` turn |
| Old `planned`, otherwise same unique owner tuple | `reserved`; old row alone is zero-send evidence |
| Old `started` or `committed`, continuation `prompt_sent`, or any contradictory active combination without positive post-I/O evidence | `dispatch_unknown`, owner quarantine, no replay |
| Unique owner plus runtime receipt with positive post-I/O prompt timestamp and matching provider session/request fingerprint | `prompt_sent`, linked turn, no replay |
| Terminal continuation with no active replay path | Preserve terminal status; map ledger only for readback, never enqueue |
| Missing/duplicate owner, mismatched attachment, failed send row, or contradictory terminal evidence | `failed` or `dispatch_unknown` according to possible bytes, quarantine, no replay |

The old engine writes `committed` and continuation `prompt_sent` before ACP I/O;
those values are therefore never positive evidence. Only the correlated
post-I/O receipt row above can produce migrated `prompt_sent`. Fixtures enumerate
every classifier input combination for all three modes and assert one terminal
classification, turn/link nullability, owner settlement, and replay exclusion.

The frozen continuation modes are `live_handle_continuation`,
`provider_session_resurrection`, and `output_only_recovery`; their prompt kinds
are the three corresponding `work_continuation_*` values. Attachment mechanism
is a separate field whose value is `live_handle` or
`provider_session_resurrection`. Output-only starts with the admitted mechanism
and may perform one CAS from live handle to resurrection after proving the
handle unavailable and before any provider-send permit. Its continuation mode,
prompt kind, instruction digest, and source-edit prohibition never change.

P086 does not reuse the generic original-prompt `runs.status = running`
predicate. Its exact permit predicate requires: a stage-owned `code_writer`
target execution in `completed` or `failed`; the target run in `running`,
`blocked`, or `failed`, with no cancellation request and no pending approval;
the admitted continuation in `running` or `preflight_passed`, not cancelling; its bound
ProcessContinuation item in Running; matching occurrence, frozen mode,
instruction digest, and reserved side-effect row; and either the matching live
generation or a successful target-bound attach receipt. A completed,
cancelled, cancelling, pending, ready, or waiting-approval run is ineligible.
Fresh `session/new` fallback without provider-session attachment is forbidden
for all three continuation modes.

The gate covers both trigger kinds and all modes, atomic rollback at every
insert boundary, idempotent replay, one-way output-only attachment conversion,
terminal parent preservation, wrong occurrence/mode/work-item rejection, and
fresh-session fallback rejection.

### Upgrade and startup ordering

`ProviderTruthUpgradeCoordinator` is part of `db::migrate::run_preflight`, not a
post-serve task. The registered SQLx migration is deliberately a staging
migration: it creates `provider_truth_upgrade_state`, shadow/final-target tables
with nullable backfill columns, and compatibility read views, but installs no
constraint that requires Rust-derived data. The current singleton daemon lock
and failed-serve boundary are held, no runtime pool or northbound listener
exists, and therefore there is a complete write fence while `Migrator::run`
finishes.

Still inside preflight, the coordinator opens `BEGIN IMMEDIATE`, records
`target_schema_version`, source schema digest, phase `backfilling`, and a stable
upgrade ID, then performs the typed Rust classifications in bounded primary-key
batches. Every batch is idempotent and advances a durable high-water mark. A
restart resumes only when source digest, target version, and upgrade ID match;
otherwise startup fails closed. Once every row is classified, one final
exclusive transaction runs `ProviderTruthSchemaFinalizer`: it rebuilds each
target table with final NOT NULL/CHECK/FK/unique constraints, copies only
validated rows, runs `foreign_key_check` plus invariant queries, switches the
compatibility views, and marks phase `complete`. No later numbered SQL migration
is needed to add those constraints. Only then does preflight open the normal
foreign-key-enforcing runtime pool. A failure in staging, backfill, final copy,
or verification leaves the daemon in failed-serve with the preflight backup and
durable phase marker; consumers never observe an interim schema.

Startup order is fixed:

1. hold all work consumers closed;
2. migrate typed envelopes, occurrence identity, prompt turns, runtime receipt
   links/states, Steward lanes, dynamic rows, and P079 lease v2 in one
   registered upgrade phase;
3. reconcile `configuring` generations and their durable process bindings,
   reaping only identity-matched children before any prompt consumer opens;
4. reconcile every pending/unknown/missing turn and block affected owner scopes;
5. assert that no replay selector can see an unresolved or unclassified prompt;
6. run existing startup recovery through the shared replay-safety query; and
7. open scheduler/continuation/steward workers only after the assertion passes.

The persisted-work matrix is normative:

| Pre-upgrade row | Upgrade result |
|---|---|
| Pending InvokeAgent, valid payload, no execution | Compile a migration-only `LegacyInvokeEnvelopeV1` from work-item ID, payload digest, durable stage owner, and frozen snapshot marker; state remains pending and provably unprompted |
| Pending InvokeAgent with malformed/missing durable owner | Mark work item `Failed`, block run/stage with `invoke_agent_upgrade_identity_missing`; do not claim |
| Running InvokeAgent with runtime receipt `handshake.prompt_sent_at_ms` | Create original turn `prompt_sent`, link receipt, preserve terminal/recovery handling, never requeue prompt |
| Running InvokeAgent with typed pre-prompt failure and no prompt timestamp | Create original turn `not_started`; existing recovery may settle it but may not replay without a newly authorized work item |
| Running InvokeAgent with absent, null, pending, or contradictory evidence | Create original turn `dispatch_unknown`, fail work item, fail only a still-running execution, and block run/stage |
| Terminal InvokeAgent/AgentExecution | Backfill readback identity when derivable; otherwise retain nullable legacy identity and never requeue |
| Historical runtime receipt with unique owner and positive send timestamp | Link/create sent turn and mark receipt `linked_v2` |
| Historical typed terminal pre-prompt receipt | Keep turn link null and mark `legacy_pre_prompt`; never infer send |
| Historical receipt with absent/ambiguous owner evidence | Keep turn link null and mark `legacy_unverified`; quarantine any running owner |
| Active v1 P079 `reserved` or `prompt_sent` | Apply the uniqueness-checked lease-v2 work-item mapping above; ambiguous rows become unknown/quarantined |
| Accepted P086 continuation with no ProcessContinuation item | Insert owner quarantine and settle continuation `failed` with `legacy_admission_enqueue_gap`; do not enqueue or prompt |
| Active P086 continuation with old provider-send ledger/status/attachment combinations | Apply every cell of `P086LegacyDispatchClassifierV1`; only correlated post-I/O receipt evidence becomes sent, all possible/contradictory delivery becomes unknown/quarantined |
| Pending StewardAnalysis | Leave pending; no Steward ACP invocation may start until upgrade reconciliation completes |
| Running StewardAnalysis work item with no pre-created analysis/lane or positive prompt evidence | Fail the work item with `steward_legacy_prompt_delivery_unverifiable`, fail an existing running analysis when present, insert `steward_work_item` quarantine, and do not replay |
| Pre-created Steward auditor lane whose system lane is terminal without a valid health report | Settle auditor `prerequisite_skipped`; settle analysis/work item after both lanes are terminal |
| Generation `configuring`, turn `not_started`, identity-matched live child | Commit `failed_before_prompt`, close/reap child, fail the owning item/lane, and retain zero-send evidence |
| Generation `configuring` with absent or ambiguous process identity | Insert process-identity quarantine, hold owner, and never signal or replay by PID alone |

`LegacyInvokeEnvelopeV1` is accepted only by the upgrade parser and cannot be
constructed or enqueued by production producers. Its compiled ID uses domain
`chainworks.legacy_compiled_task.v1` and exact components
`[work_item_id, payload_sha256, owner_kind, owner_id, workflow_identity_marker]`;
its occurrence still uses
`chainworks.task_occurrence.v1`. Conversion writes a validated
`InvokeAgentEnvelopeV1` whose required provenance union is
`legacy_migration`, never an ordinary production envelope with an extra field.
The upgrade ID, untouched payload digest, owner tuple, and frozen marker are
validated against the source row in the same transaction. Production enqueue
has no constructor for that union variant. This is deterministic without
rewriting workflow/catalog snapshots.

All requeue/retry/fallback/continuation selectors call one DB-owned
`PromptReplaySafety::with_safe_owner_tx` API. It starts and owns the SQL
transaction, rejects unresolved unknown, stale pending, missing authoritative
turn, active quarantine, owner mismatch, or migration-pending rows, and invokes
the caller closure with a private `ReplaySafetyTx<'tx>` that exclusively borrows
that connection. Prompt-capable enqueue/status methods are methods on this
wrapper; they accept no pool, external transaction, or reusable proof token.
The wrapper carries a closed `ReplaySelectorIdV1` and cannot be returned from
the higher-ranked closure, cloned, serialized, or moved into another
transaction. Commit occurs only after the closure succeeds. Compile-fail tests
cover attempted token return/cross-transaction reuse, while runtime tests cover
rollback after authorization and a racing owner mutation.

The enum-generated manifest classifies, at minimum, normal invoke claim;
preclaimed/startup/stale-starting/pre-session invoke requeue; targeted advance
and retry-authority requeue; host-interruption, active-prompt-close,
provider-capacity-wait, and persistence-contention requeue; automatic contract
retry; P058 escalation/deadline resume; command stage/agent retry; P079 repair;
P086 admission/claim; provider fallback; Steward startup requeue; and normal
Steward claim. Private `_tx` delegates inherit the public selector ID and cannot
mint a checked value.

A `syn`-based retained gate generates the manifest from all calls that enqueue
InvokeAgent/ProcessContinuation/StewardAnalysis, change one of those work items
to Pending or Running, claim one, or call the manager's authorized prompt API.
It compares the result byte-for-byte with the enum manifest and recursively
rejects raw status SQL, generic prompt-capable enqueue, or an unclassified call
site outside the owning repositories. This makes a new production retry route
a compile/gate failure rather than a manually remembered list entry.

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

`ProviderConfigurationReceiptV1` is owner-scoped and has exactly these
keys: `schema_version` with literal value
`provider_configuration_receipt_v1`,
`provider_configuration_contract_version`, `configuration_owner_kind`,
`configuration_owner_id`, `configuration_attempt_index`, nullable `agent_execution_id`, nullable
`task_occurrence_id`, `work_item_id`, nullable `session_generation_id`, nullable
`provider_session_id`, `provider`, nullable `binding_fingerprint_sha256`,
`requested_model`, `requested_effort`, nullable `accepted_model`, nullable
`accepted_effort`, nullable `accepted_model_wire_value`, nullable
`accepted_effort_wire_value`, `configuration_state`, nullable
`acceptance_source`, nullable `source_generation_acceptance_sha256`, nullable
`verified_at`, nullable `failure_code`, and the non-negative integer
`prompt_dispatch_count_at_receipt`.

`configuration_attempt_index` and `prompt_dispatch_count_at_receipt` are
non-negative integers; the former must equal the owner's allocated attempt.

Receipt owner kind is closed to `agent_execution` or `steward_agent_lane`;
receipt configuration state is closed to `configured`,
`failed_before_prompt`, or `cancelled_before_prompt`; acceptance source, when
present, is `fresh_negotiation` or `reused_session_generation`. A configuring or
legacy projection has no receipt yet. These domains are JSON Schema enums, not
free strings.

For `configured`, both session IDs, binding fingerprint, all accepted/source
fields, source digest, and verification time are non-null, `failure_code` is
null, and the prompt count is zero. For `failed_before_prompt` or
`cancelled_before_prompt`, accepted/source/digest/time fields are null,
`failure_code` is non-null, the prompt count is zero, and session IDs plus the
binding fingerprint may be null only when settlement precedes their creation.
No unknown JSON keys are accepted. For owner kind `agent_execution`, both
execution/occurrence fields are non-null and match the owning execution row.
For `steward_agent_lane`, both are null and the owner ID is exactly the durable
lane ID whose analysis, agent, provider, and work item match the invocation.
All receipt work-item and requested values must equal owner truth; all
configured generation fields must equal the referenced generation acceptance.

The generation digest is lowercase hex SHA-256 over UTF-8 RFC 8785 canonical
JSON of `ProviderConfigurationAcceptanceV1`; the digest itself is stored beside
and excluded from that object. `ProviderConfigurationAuthority` in engine is
the sole encoder/verifier. It recomputes the digest before generation insert,
before reuse projection, and when loading an active generation. Digest mismatch
or malformed/oversized JSON invalidates the generation and returns
`ACP_PROVIDER_CONFIGURATION_EVIDENCE_INVALID` before prompt dispatch.

The owner receipt has an independent lowercase SHA-256 over the RFC 8785 bytes
of `ProviderConfigurationReceiptV1`. `provider_configuration_receipts` stores
it as `receipt_sha256`; an AgentExecution projection stores the same digest.
The digest is not a JSON member. Authority recomputes it before insert, runtime
receipt validation, and readback projection; a JSON/digest or authority/projection
mismatch fails closed with the same evidence-invalid code.

Input is parsed by a duplicate-key-rejecting JSON visitor before RFC 8785
canonicalization; ordinary `serde_json::Value` last-key-wins parsing is not
allowed on this boundary. Normative known-answer fixtures include input
`{"z":0,"a":[3,2,1]}`, canonical bytes `{"a":[3,2,1],"z":0}`, and SHA-256
`3f924cf502119a296b4c209a3192b12997b63d4e1b2e7d34eea488b9c0b831c2`, plus
the RFC 8785 number/string vector. `{"a":1,"a":2}` must be rejected before
hashing.

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
complete v1 shape, and adds three required keys:
`provider_configuration_receipt`, `provider_configuration_failure`, and
`prompt_turn`. For an exact-contract Codex attempt, exactly one of the first two
is non-null. The receipt branch is a valid `ProviderConfigurationReceiptV1`;
the failure branch is `ProviderConfigurationFailureV1` with exact keys
`schema_version`, owner kind/ID, attempt index, work-item ID, nullable
generation/process binding, failure code, cleanup state, and occurred-at. Both
are null for non-Codex or legacy-v0 owners. `prompt_turn` has exactly non-null `prompt_turn_id`,
`prompt_kind`, `prompt_owner_kind`, `prompt_owner_id`, non-negative integer
`turn_index`, and `dispatch_state` in `not_started`, `dispatch_pending`,
`prompt_sent`, or `dispatch_unknown`; unknown keys are rejected. The canonical
v2 encoder emits every v1 top-level key:
nullable values are explicit null, arrays and booleans are explicit, and no
unknown top-level key is allowed. Decoder behavior is frozen:

| Runtime receipt input | Result |
|---|---|
| integer `schema_version = 1` | Decode as legacy; all three new fields unavailable |
| v2 with all keys and exactly one valid configuration branch/reference | Decode, authority-verify, then project |
| v2 with an omitted key, both/neither exact-Codex branches, malformed nested object, or authority digest mismatch | `ACP_RUNTIME_RECEIPT_INVALID` |
| Any unsupported schema version | `ACP_RUNTIME_RECEIPT_UNSUPPORTED_VERSION` |

The implementation adds normative `additionalProperties: false` schemas at
`docs/reference/schemas/provider-configuration-acceptance-v1.schema.json`,
`docs/reference/schemas/provider-configuration-receipt-v1.schema.json`,
`docs/reference/schemas/provider-configuration-failure-v1.schema.json`, and
`docs/reference/schemas/acp-runtime-receipt-v2.schema.json`, plus valid/invalid
fixtures. For run agents, the execution-row projection, owner-scoped
configuration receipt, authoritative prompt turn, and every terminal runtime
receipt must agree on execution, occurrence, turn/owner tuple,
requested/accepted pair, source digest, generation, and provider-session
binding. For Steward, the owner-scoped receipt and prompt turn must agree on
analysis/lane/agent owner, work item, requested pair, generation, and provider
session; no execution projection is invented. The durable prompt-turn row
remains dispatch authority; a v2 receipt
is accepted only when its tuple and observed state equal that row and it never
mutates the row's dispatch state.
`ProviderConfigurationAuthority` performs the database-backed source-generation
digest comparison after structural decode and before any readback projection.

### Configuration and prompt-turn dispatch lifecycle

The engine inserts a fresh exact Codex execution with requested fields and
`provider_configuration_state = configuring` before ACP startup. Claim/start
atomically creates the execution and its `original/0` turn in
`not_started`; non-Codex and legacy executions receive the same original row
with non-applicable/unverified configuration truth. For Steward,
`run_steward_analysis_with_executor` loads the pre-inserted analysis and two
lane rows, then threads the lane ID and claimed StewardAnalysis work-item ID
into each `StewardAgentInvocation`; the executor reserves
`steward_analysis/0` under that lane before calling ACP. It does not manufacture
a RunId, StageExecution, or AgentExecution as authority. A strict owner-aware
provider-configuration sink on
`AcpRuntimeManager`:

- after both option responses are verified, atomically writes generation
  acceptance and the owner receipt, then projects it and marks configuration
  `configured` when the owner is an AgentExecution;
- on negotiation failure, writes `failed_before_prompt` with null accepted
  fields and a typed receipt;
- on cancellation that wins before configuration completes, writes
  `cancelled_before_prompt`, keeps the original turn `not_started`, and marks
  the execution/work item cancelled in the same settlement transaction;
- returns `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` with zero prompts if
  authority persistence fails.

The crate boundary is dependency-compatible. `acp` defines two object-safe
ports and owns all live handles/process supervision:

- `PromptDispatchAuthorityPort` exposes owner/configuration settlement and the
  initial/final turn CAS; engine's `DurablePromptDispatchAuthority` implements
  it using `db` and is injected into the manager; and
- `ProviderRuntimeControlPort` exposes generation-scoped cancel/interrupt/reap
  results without DB types; `AcpRuntimeManager` implements it and engine injects
  that port into `DispatchInvalidationCoordinator`.

Thus `acp` never depends on engine/db, while engine's existing dependency on
`acp` is sufficient for both trait definitions and runtime control. Daemon is
the composition root: it creates the durable authority, constructs exactly one
manager with it, and constructs the coordinator with the manager's control
port. The coordinator owns DB cancellation/epoch/owner transitions; the manager
owns tokens, handles, process groups, and transport. Neither port exposes a raw
session handle or permits ACP to mutate DB directly.

The manager's only public prompt-capable entry points are `execute_authorized`
and `continue_authorized`; both require a validated owner request and call the
authority immediately before transport write. The authority's successful CAS
is consumed inside the manager to create a private, non-`Clone`, one-use
`PromptDispatchPermit`. No caller can construct or receive that permit.

`AcpSession`, `AcpSessionHandle`, adapter/session constructors, `start_session`,
`prompt_session`, raw `execute`, and every `prompt*` method become `pub(crate)`
and are removed from `acp::lib` re-exports. Attach-only resurrection may remain
public only as a no-prompt operation; the attached handle is opaque and can be
consumed solely by `continue_authorized`. Fixture bypasses are compiled under
`cfg(test)` or a non-daemon test feature. A recursive public-API and call-site
gate permits exactly one production `session/prompt` transport write site,
inside the manager after permit consumption, and fails on any raw alternative
or fallback dispatch.

Every prompt turn independently follows:

```text
not_started -> dispatch_pending
dispatch_pending -> prompt_sent
dispatch_pending -> dispatch_unknown
```

`PromptDispatchOwnerV1` freezes owner-specific predicates:

| Prompt kind | Owner | Required permit truth | Unknown settlement |
|---|---|---|---|
| `original` | InvokeAgent work item | Matching execution/occurrence and InvokeAgent item are running | Fail running execution and item; block stage/run |
| `code_writer_completion_repair` | P079 lease key | Parent execution/occurrence match; lease and InvokeAgent item are active | Mirror lease unknown; fail running item/execution; block stage/run |
| `output_contract_repair` | P079 lease key | Same P079 owner proof with generic repair event kind | Same P079 unknown settlement |
| `work_continuation_live_handle` | P086 continuation ID | Target execution/occurrence match even if execution is terminal; ProcessContinuation item running; continuation active and not cancelling | Mark continuation `needs_continuation_reconciliation`, fail item, preserve terminal parent execution, block stage/run |
| `work_continuation_resurrection` | P086 continuation ID | Same as live handle plus successful target-bound attach receipt | Same P086 settlement and close attached generation |
| `work_continuation_output_only` | P086 continuation ID | Frozen output-only mode, selected attachment proof, ProcessContinuation item running, source-edit prohibition frozen | Same P086 settlement; retain output-only evidence |
| `steward_analysis` | Steward lane ID | Matching StewardAnalysis item and lane are active; invocation carries the same analysis, lane, agent, provider, and work item; no prior turn exists | Mark lane `prompt_delivery_unknown`, fail the work item/analysis, preserve deterministic inputs, and forbid automatic replay |

All three P086 paths must pass the target `agent_execution_id`, its durable
occurrence, allocated prompt turn, and ProcessContinuation work-item ID into
`ExecutionRequest`; `agent_execution_id = None` is rejected for a run-bound
prompt. Provider-session attach itself sends no prompt and therefore has no
turn, but it cannot authorize the resurrection turn until identity proof is
durable. P086 continuation admission atomically reserves the turn and inserts
its `provider_send` side-effect row as `reserved`, storing `prompt_turn_id`.
Permit, flush, and ambiguity transactions mirror that side-effect row to
`dispatch_pending`, `prompt_sent`, or `dispatch_unknown`. Continuation status
may become `prompt_sent` only in the successful final CAS, never before ACP I/O;
replay checks the canonical turn first and cannot mint a second side effect.

The P079 lease-v2 transitions described above occur in the same transactions as
its allocated turn. P079, P086, and Steward are not independent send
authorities; their domain rows are owner projections of
`provider_prompt_turns`. The Steward executor must not call the permit-requiring
ACP prompt API with `agent_execution_id = None` unless it supplies the validated
`steward_agent_lane` owner tuple and `steward_analysis` prompt kind above.

`AcpRuntimeManager` owns one async `SessionPromptGate`, one cancellation token,
and one absolute `ConfigurationDeadline` per live generation from allocation
onward. The deadline is 30 seconds total and is never reset. Spawn/barrier
release, `initialize`, `session/new`, model write, model readback, effort write,
and final model/effort readback each run under `tokio::select!` against that
deadline and token. Timeout/cancellation records the exact last phase, invokes
deterministic coordinator cleanup, and settles zero-send configuration failure;
an identity-ambiguous child is quarantined rather than signalled. No detached
task may outlive cleanup. Configuration settlement uses a CAS over the captured
run epoch and owner status. Prompt dispatch then holds the gate from permit
through its separate fixed 10-second transport write/flush deadline and final
CAS, serializing multiple turns without allowing cancellation to wait
indefinitely.

Engine's `DispatchInvalidationCoordinator` is the only entry point for run cancellation,
stage/execution replacement, targeted retry cancellation, work-item
cancellation, direct session close, daemon shutdown, and resurrection cleanup.
It first commits the owner cancellation/supersession intent; run-wide
invalidation also increments `runs.prompt_dispatch_epoch`, while scoped
invalidation changes only the affected owner records. It then signals the
generation cancellation token without acquiring the prompt gate. Transport
write/flush runs under `tokio::select!` with that token and a fixed 10-second
write deadline and reports `zero`, `some`, or `unknown` bytes written. The
coordinator waits at most the same deadline for the gate, then asks the
supervised-process owner to interrupt/kill the provider out of band and settles
any pending turn unknown. Only after settlement does it remove the live handle.
Raw `close_session`, `request_close_session`, `close_all_sessions`, adapter kill,
cancel, and supersede APIs become private to the coordinator.

If invalidation commits before the initial permit, epoch/owner CAS prevents all
bytes. If permit commits first, later invalidation may race with I/O and the
result is `dispatch_unknown` unless final `prompt_sent` committed first. This
ordering avoids deadlock and never claims zero bytes merely because cancellation
was requested.

Initial permit and final sent CAS both return `Applied`, `AlreadyMatching`,
`Conflict`, or `Missing`, with distinct meaning:

| Boundary/result | Durable action | May send/replay? |
|---|---|---|
| Initial `Applied` | `not_started -> dispatch_pending`; mirror owner ledger | Send once in this guard |
| Initial `AlreadyMatching` | Prior pending commit may have lost its ack; settle unknown | No |
| Initial `Conflict` | Owner/epoch/generation/config predicate lost; typed prepare failure with turn unchanged | No |
| Initial `Missing` | Authoritative turn absent; fail owner with `prompt_turn_missing`, block stage/run | No |
| Final `Applied` | `dispatch_pending -> prompt_sent`; persist flush time and mirror owner ledger | Complete |
| Final `AlreadyMatching` | Exact sent row proves final commit ack was lost; treat as idempotent success | Complete, no replay |
| Final `Conflict` | Bytes may have left but owner/state differs; settle unknown | No |
| Final `Missing` | Bytes may have left and authority vanished; recreate only quarantine evidence, settle owner unknown | No |

Crash/restart behavior is frozen at each durable boundary:

| Last durable boundary | Startup action | Prompt replay |
|---|---|---|
| Owner/turn reserved; no launch intent | Settle typed preparation failure; original/Steward owner may use its existing bounded zero-send infrastructure retry policy | Only through a new checked claim |
| `spawn_pending`; launch barrier not released | Observe barrier EOF/child absence, settle zero-send launch failure | Same zero-send policy |
| PID/start identity persisted; barrier released; no `session/new` result | Identity-check and reap child, settle configuration failure | No reuse of old generation |
| `session/new` or configuration `configuring`; turn `not_started` | Identity-check and reap, write `failed_before_prompt`; ambiguous identity quarantines owner | No P079/P086 fresh fallback |
| Configured receipt committed; turn `not_started`; daemon lost transport | Reap old generation; original/Steward may renegotiate under a new generation and checked claim, P079/P086 fail closed | Never reuse old receipt as new-generation truth |
| Initial permit committed (`dispatch_pending`) | Close/reap and settle `dispatch_unknown` plus quarantine | Never |
| Transport write/flush started, final CAS absent | Close/reap and settle `dispatch_unknown` plus quarantine | Never |
| Final `prompt_sent` CAS committed, acknowledgement lost | Treat exact `AlreadyMatching` as sent and reconcile output/terminal receipt | Never resend |
| Terminal receipt committed, owner settlement absent | Verify receipt/turn tuple and idempotently finish owner settlement | Never resend |

The retained crash harness kills the daemon after each row above for original,
both P079 prompt kinds, all three P086 modes, and both Steward lanes. It asserts
one surviving process owner, no identity-unverified signal, no duplicate prompt,
and deterministic owner settlement.

The initial CAS binds turn ID/kind/index/owner, owning running work item, active
generation/provider session, contract/requested pair, owner-specific state
above, and no cancelling provider intent. Run-bound owners additionally bind
execution, occurrence, captured run epoch, and their row in the owner-specific
predicate table; there is no shared `runs.status = running` shortcut. Steward
instead binds analysis ID, lane ID, and agent ID and has no run epoch. Exact Codex
additionally requires configured accepted truth matching the generation. Only
initial `Applied` yields an opaque
single-use `PromptDispatchPermit`; `AcpRuntimeManager` prompt APIs require that
permit, so direct sends cannot bypass durable authority.

Crash, timeout, cancellation after permit, send/flush error, or final ambiguity
closes the generation and applies the owner-specific unknown settlement. Startup
does the same for stale pending turns. Every selector delegates to
`PromptReplaySafety` and excludes unresolved unknown turns.

P079 repair must reuse the parent generation proved by its lease and P086 must
use its admitted live/attached generation. Neither owner kind may fall through
to the manager's fresh `session/new` path. Fresh-session creation is available
only to an original prompt owner whose frozen invocation contract explicitly
allows it.

Run-bound terminal runtime-receipt persistence inserts into
`agent_execution_runtime_receipts` with the exact `prompt_turn_id`. It cannot
advance or overwrite dispatch state; a conflicting insert fails closed and
leaves the turn for startup reconciliation.

A stale `not_started` turn with no matching live generation is provably
unprompted and may settle `prompt_dispatch_preparation` without ambiguity.
`No prompt sent` is derived only when no turn for the execution advanced past
`not_started`; it is not inferred from Codex configuration state. `Using` or
`Used` requires original turn `prompt_sent`. Repair and continuation turns are
reported separately, and unresolved `dispatch_unknown` dominates all aggregate
copy.
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

The existing async-graphql names are normative: `QueryRoot`,
`GqlAgentExecution`, `GqlMediationExecutionAttempt`,
`RunStageTopologyOccurrence`, and `RunStageTopologyTransition`. The following
is an explicitly abbreviated additive delta, not a complete SDL document. The
complete checked-in `AppSchema::sdl()` output is the schema authority and the
gate byte-compares a fresh generation to that snapshot.

```graphql
enum ProviderConfigurationState {
  CONFIGURING CONFIGURED FAILED_BEFORE_PROMPT CANCELLED_BEFORE_PROMPT
  LEGACY_UNVERIFIED
}
enum ProviderPromptDispatchState {
  NOT_STARTED DISPATCH_PENDING PROMPT_SENT DISPATCH_UNKNOWN
}
enum ProviderPromptKind {
  ORIGINAL CODE_WRITER_COMPLETION_REPAIR OUTPUT_CONTRACT_REPAIR
  WORK_CONTINUATION_LIVE_HANDLE WORK_CONTINUATION_RESURRECTION
  WORK_CONTINUATION_OUTPUT_ONLY STEWARD_ANALYSIS
}
enum ProviderPromptOwnerKind {
  INVOKE_AGENT P079_REPAIR P086_CONTINUATION STEWARD_AGENT_LANE
}
enum ProviderConfigurationAcceptanceSource {
  FRESH_NEGOTIATION REUSED_SESSION_GENERATION
}
enum RuntimeReceiptLinkState {
  LINKED_V2 LEGACY_PRE_PROMPT LEGACY_UNVERIFIED
}
enum ProviderConfigurationEvidenceState {
  PENDING RECEIPT_AVAILABLE RECEIPT_UNAVAILABLE NOT_APPLICABLE LEGACY_UNVERIFIED
}
enum ProviderPromptDeliveryTruth {
  NOT_STARTED ORIGINAL_PENDING ORIGINAL_SENT REPAIR_PENDING REPAIR_SENT
  CONTINUATION_PENDING CONTINUATION_SENT UNKNOWN LEGACY_UNVERIFIED
}
type ProviderPromptTurn {
  promptTurnId: ID!
  promptKind: ProviderPromptKind!
  turnIndex: Int!
  promptOwnerKind: ProviderPromptOwnerKind!
  promptOwnerId: ID!
  dispatchState: ProviderPromptDispatchState!
  dispatchStartedAt: String
  promptSentAt: String
  dispatchUnknownAt: String
  failureCode: String
  runtimeReceiptLinkState: RuntimeReceiptLinkState
}
type RuntimeReceiptLinkSummary {
  linkedV2Count: Int!
  legacyPrePromptCount: Int!
  legacyUnverifiedCount: Int!
  worstState: RuntimeReceiptLinkState
}
type ProviderPromptDispatchSummary {
  originalTurnState: ProviderPromptDispatchState
  latestTurnKind: ProviderPromptKind
  latestTurnIndex: Int
  latestTurnState: ProviderPromptDispatchState
  deliveryTruth: ProviderPromptDeliveryTruth!
  noPromptSent: Boolean!
  hasUnresolvedUnknown: Boolean!
}
extend type QueryRoot {
  providerExecutionTruthSchemaVersion: Int!
}
extend type GqlAgentExecution {
  taskOccurrenceId: ID
  requestedModel: String
  requestedEffort: String
  acceptedModel: String
  acceptedEffort: String
  providerConfigurationState: ProviderConfigurationState
  configurationEvidenceState: ProviderConfigurationEvidenceState!
  acceptanceSource: ProviderConfigurationAcceptanceSource
  providerConfigurationVerifiedAt: String
  runtimeReceiptLinkSummary: RuntimeReceiptLinkSummary!
  promptDispatchSummary: ProviderPromptDispatchSummary!
  promptTurns: [ProviderPromptTurn!]!
}
extend type RunStageTopologyOccurrence {
  presentationRowId: ID!
  compiledTaskId: ID!
  taskOccurrenceId: ID
  activeExecutionId: ID
  executionProvider: String
  requestedModel: String
  requestedEffort: String
  acceptedModel: String
  acceptedEffort: String
  providerConfigurationState: ProviderConfigurationState
  configurationEvidenceState: ProviderConfigurationEvidenceState!
  runtimeReceiptLinkSummary: RuntimeReceiptLinkSummary!
  executionAssociationState: TopologyExecutionAssociationState!
  legacyAmbiguousExecutionCount: Int!
  promptDispatchSummary: ProviderPromptDispatchSummary!
}
enum TopologyExecutionAssociationState {
  MATCHED_V2 LEGACY_UNIQUE LEGACY_AMBIGUOUS NOT_STARTED
}
enum TopologyOccurrenceSourceKind {
  STATIC_COMPILED OWNER_COMPILED DYNAMIC_MATERIALIZED LEGACY_FLAT
}
extend type RunStageTopologyNode {
  frozenWorkflowOrdinal: Int!
  legacyOrderUnverified: Boolean!
}
extend type RunStageTopologyOccurrence {
  sourceKind: TopologyOccurrenceSourceKind!
  sourceStableId: ID!
  frozenTaskOrdinal: Int!
}
extend type RunStageTopologyTransition {
  transitionId: ID!
  transitionOrdinal: Int!
}
extend type GqlMediationExecutionAttempt {
  taskOccurrenceId: ID
  requestedModel: String
  requestedEffort: String
  acceptedModel: String
  acceptedEffort: String
  providerConfigurationState: ProviderConfigurationState
  configurationEvidenceState: ProviderConfigurationEvidenceState!
  acceptanceSource: ProviderConfigurationAcceptanceSource
  providerConfigurationVerifiedAt: String
  runtimeReceiptLinkSummary: RuntimeReceiptLinkSummary!
  promptDispatchSummary: ProviderPromptDispatchSummary!
  promptTurns: [ProviderPromptTurn!]!
}
```

`GqlMediationExecutionAttempt` adds the same nullable execution/configuration
scalars, non-null summary, and non-null turn list as `GqlAgentExecution`. IDs,
turn fields, enums, summary containers, lists, and booleans above are non-null;
only fields shown without `!` are nullable. Historical AgentExecution rows may
have null occurrence/configuration scalars. Topology always derives non-null
presentation/compiled IDs from the frozen or migration identity even when no
execution exists.

An unresolved unknown turn wins aggregation. Otherwise the greatest turn index
is latest, while original sent truth remains independently available. A
historical execution with an empty turn list freezes
`deliveryTruth = NOT_STARTED` and `noPromptSent = true` only when its terminal
runtime receipt is `legacy_pre_prompt`. Otherwise it freezes
`deliveryTruth = LEGACY_UNVERIFIED`, `noPromptSent = false`, and
`hasUnresolvedUnknown = false`; all original/latest fields are null. A planned
topology occurrence with no execution freezes `NOT_STARTED`, true, false. For a
non-empty list, `noPromptSent` is true only when every turn is `NOT_STARTED`.

Receipt linkage is never collapsed to an unexplained scalar. Each linked turn
exposes its own nullable `runtimeReceiptLinkState`; unlinked historical receipts
remain outside the turn array. `runtimeReceiptLinkSummary` counts all execution
receipts, including those unlinked rows, and computes `worstState` by the total
order `LEGACY_UNVERIFIED > LEGACY_PRE_PROMPT > LINKED_V2`; it is null only when
the execution has no runtime receipts. Counts are non-negative and sum to the
receipt row count. Thus original, both repair kinds, and every continuation can
have different link truth without losing evidence.

Execution association is explicit. A v2 occurrence match is `MATCHED_V2`; the
single-task historical `agent_id` fallback is `LEGACY_UNIQUE`; more than one
candidate is `LEGACY_AMBIGUOUS`, leaves `activeExecutionId` and all runtime
identity fields null, and reports the bounded candidate count; no candidate is
`NOT_STARTED`. The latest execution within a matched occurrence is selected by
the total order `started_at DESC, id DESC` using canonical RFC 3339 UTC bytes
and lowercase UUID bytes. No database row order or `selection_order` breaks a
tie.

Every frozen transition receives zero-based `transitionOrdinal` in source YAML
order and `transitionId = topology_transition_v1:<sha256>` over exact components
`[workflow_snapshot_sha256, from_state_id, transition_ordinal, to_state_id, canonical_condition_sha256]`
with the common codec. Layout edge ordering is
`transitionOrdinal ASC, transitionId ASC`; shuffled SQL results cannot reorder
branches. `ConditionCanonicalV1` encodes the parsed transition AST, never source
whitespace. A node is common-codec components: `exists` is
`["exists", artifact_name]`; comparison is
`["compare", operator, left_operand, right_operand]`; `and`/`or` is
`[tag, child_count, child_digest_0, ...]` in source order. An operand is
`path:<exact dotted path>` or `json:<RFC8785 literal bytes>`. Parentheses vanish
after parsing; commutative children are not reordered. Each child digest and the
final digest use domain `chainworks.transition_condition.v1`. Unconditional is
the digest of common-codec components `["always"]`, not a platform empty-string
hash. Ordinals come from the frozen parsed transition sequence, never a map
iteration. Checked-in vectors cover whitespace/parenthesis equivalence,
different child order, Unicode literal bytes, numeric canonicalization, and a
parse failure.

Its existing `provider`, `model`, and `effort` fields continue to mean frozen
planned identity for compatibility. The new fields come only from the latest
execution matched by occurrence ID. Retry/fallback cannot overwrite another
same-agent task.

One domain-owned serializer for the purpose-built, sanitized nested
`provider_execution_truth_v1` and
`docs/reference/schemas/provider-execution-truth-v1.schema.json` own MCP,
mediation, and report parity. It has required keys `schema_version`, nullable
`agent_execution_id`, nullable `task_occurrence_id`, nullable
`execution_provider`, nullable requested/accepted pair,
`provider_configuration_state`, non-null `configuration_evidence_state`,
`acceptance_source`, `provider_configuration_verified_at`, non-null
`runtime_receipt_link_summary`,
required `prompt_dispatch_summary`, and
required sanitized `prompt_turns`; nullable keys are always present as explicit
null and `additionalProperties` is false. Prompt summary/turn objects use
snake-case fields and lowercase enum wire values. This DTO contains no raw
runtime receipt, provider-session ID, prompt text, artifact payload, permission
history, escalation diagnostics, or internal row object.

Outer placement includes the actual MCP envelope, not merely a path inside its
decoded text:

| Surface | Wire envelope | Path after JSON-decoding text |
|---|---|---|
| `run://{run_id}` | `result.contents[0].text` | `stages[*].agent_executions[*].provider_execution_truth` |
| `report://{run_id}` | `result.contents[0].text` | `agent_executions[*].provider_execution_truth` |
| `reports.get` | `result.content[0].text` and equal object at `result.structuredContent` | `reports[* where report_kind = "mcp_execution_truth"].agent_executions[*].provider_execution_truth` |
| MCP mediation readback | Same resource/tool envelope as its parent | `workflow_conflict.lead_mediation.execution_attempts[*].provider_execution_truth` |
| Generated `run_report` JSON artifact | Filesystem artifact bytes | top-level `provider_execution_truths[*]`, ordered by `agent_execution_id` |
| GraphQL | Ordinary GraphQL `data` envelope | typed fields on `GqlAgentExecution` and `GqlMediationExecutionAttempt` |

For `reports.get`, `content[0].text` is canonical JSON of
`structuredContent`; the two decode to equal values. Resources keep the MCP
`contents` envelope. Envelope fixtures exercise JSON-RPC `result`, MIME type,
text decoding, missing/wrong content indices, and structured-content mismatch.

GraphQL wire spelling is intentionally not byte-equal to shared JSON. The
generated mapper is a bijection: snake-case JSON fields map to lower-camel
GraphQL fields, lowercase enum values map to their exact SCREAMING_SNAKE GraphQL
members, null maps to null, and arrays preserve canonical order. For example,
`runtime_receipt_link_summary.worst_state = "legacy_pre_prompt"` maps to
`runtimeReceiptLinkSummary.worstState = LEGACY_PRE_PROMPT`. The gate first
byte-compares canonical domain JSON across MCP/report/artifact surfaces, then
maps a GraphQL response back to the domain DTO and compares that canonical JSON.
It never demands impossible raw GraphQL/JSON byte equality.

`reports.get` and `report://{run_id}` must embed the same serialized object for
the same execution; the generated run report stores the same objects rather
than a locally shaped attempt copy. Existing `model` remains a requested-value
compatibility field and is never labeled runtime/accepted truth.
Provider-session IDs and raw receipt JSON retain their operator-only redaction
boundary.

The lead agent remains author of the semantic run-report candidate, but cannot
author execution truth. `engine::RunReportMaterializer` is the sole final writer
of logical artifact `run_report`. After candidate validation and after all
AgentExecution settlement, it acquires the existing artifact-writer lease,
removes any agent-supplied `provider_execution_truths`, inserts the authoritative
ordered array from the domain serializer, and canonicalizes the complete JSON.
Before replacement it persists a materialization journal with candidate hash,
truth-array hash, intended final hash, target path, and state `prepared`. It
writes a same-directory temporary file, fsyncs it, atomically renames, fsyncs the
directory, then transactionally updates artifact metadata/checksum and journal
state `committed`. Startup verifies a prepared row against target/temp hashes
and either completes the exact rename/metadata commit or restores the validated
candidate; it never lets the lead overwrite a committed report. Tests kill at
every journal/write/fsync/rename/metadata boundary and prove one final checksum
and byte-equal embedded DTOs.

`run://` no longer serializes a database `AgentExecution` directly. A
`RunAgentExecutionReadbackV1` allowlists existing non-sensitive summary fields
plus `provider_execution_truth`; all diagnostic/session/receipt/prompt/artifact
payload fields are absent by type. `report://` and `reports.get` remain
Operator-only under the existing report authorization boundary. GraphQL uses
the same safe DTO for Agent/Observer callers and adds operator diagnostics only
through separately authorized fields. Fixtures cover Operator, Agent, and
Observer, cross-run scope denial, and assert absence of provider-session IDs,
raw receipts, completion/workflow/retry/P091/rollout diagnostics, and artifact
payload lanes before serialization.

Steward prompt turns are not projected into run topology or AgentExecution
GraphQL because they are not run executions. Existing Steward analysis/report
readback may expose only its typed lane outcome and sanitized owner-scoped
configuration/dispatch summary; it must not fabricate run, stage, occurrence,
or execution identifiers.

Swift DTOs declare every `CodingKey` and use a custom decoder that distinguishes
`container.contains(key) == false` (typed schema mismatch) from explicit null
(valid state). Checked-in GraphQL, shared-JSON, MCP, report, and Swift fixtures
cover historical Codex, non-Codex, pre-session configuration failure,
mediation, both P079 repairs, all three P086 continuation modes, empty legacy turns, and
schema mismatch.

### Closed public values and nullability

The JSON Schema, Rust enums, GraphQL enums, and Swift enums are generated or
byte-compared from these closed wire domains:

| Domain | Exact JSON values |
|---|---|
| configuration state | `configuring`, `configured`, `failed_before_prompt`, `cancelled_before_prompt`, `legacy_unverified` |
| configuration evidence | `pending`, `receipt_available`, `receipt_unavailable`, `not_applicable`, `legacy_unverified` |
| effective provider capability | `codex_exact_pair_v1`, `not_applicable_v1`, `legacy_best_effort_v0` |
| acceptance source | `fresh_negotiation`, `reused_session_generation` |
| prompt kind | `original`, `code_writer_completion_repair`, `output_contract_repair`, `work_continuation_live_handle`, `work_continuation_resurrection`, `work_continuation_output_only`, `steward_analysis` |
| prompt owner kind | `invoke_agent`, `p079_repair`, `p086_continuation`, `steward_agent_lane` |
| prompt dispatch state | `not_started`, `dispatch_pending`, `prompt_sent`, `dispatch_unknown` |
| delivery truth | `not_started`, `original_pending`, `original_sent`, `repair_pending`, `repair_sent`, `continuation_pending`, `continuation_sent`, `unknown`, `legacy_unverified` |
| runtime-receipt link | `linked_v2`, `legacy_pre_prompt`, `legacy_unverified` |
| topology association | `matched_v2`, `legacy_unique`, `legacy_ambiguous`, `not_started` |
| topology occurrence source | `static_compiled`, `owner_compiled`, `dynamic_materialized`, `legacy_flat` |

Unknown enum strings are schema errors, not display fallbacks. Nullability is
also normative:

- a new `codex_exact_pair_v1` execution always has requested model and effort;
  historical and non-Codex requested fields retain their existing optionality;
- `configured` requires accepted model/effort, wire pair, acceptance source,
  generation/session binding, receipt digest, verified time, and
  `receipt_available` together;
- `configuring`, either pre-prompt terminal state, and `legacy_unverified`
  require all accepted/source/verified fields null;
- non-Codex execution requires configuration state, accepted wire pair,
  acceptance source, and configuration receipt null, and evidence
  `not_applicable`;
- `receipt_unavailable` requires `failed_before_prompt`, a null receipt pointer,
  a matching configuration-failure row, and a `not_started` prompt turn;
- every execution-shaped DTO has a non-null prompt summary and non-null turn
  array, including an empty legacy array; topology with no execution has null
  execution truth plus non-null `NOT_STARTED` summary, with configuration
  evidence `pending` for planned exact Codex, `not_applicable` for planned
  non-Codex, or `legacy_unverified` for a legacy row;
- every prompt turn's receipt-link state is null when no receipt links to that
  turn; the non-null link summary counts linked and unlinked receipts, sums to
  total receipt count, and uses the frozen worst-state order;
- receipt JSON always emits every declared nullable key as explicit null;
  GraphQL omits none of the declared fields, and Swift treats omission as a
  schema mismatch.

The single schema fixture enumerates every legal state/nullability row and
mutation-negatives for one missing key, one unknown enum, one half-populated
accepted pair, configured-without-receipt, legacy-with-accepted-values, and
non-Codex configuration leakage.

### Lockstep daemon schema

GraphQL rejects a document containing unknown fields; an old daemon does not
return those fields as `nil`. The updated app therefore requires lockstep
replacement of the bundled daemon rather than issuing a reduced legacy run
detail query.

The probe SDL is `providerExecutionTruthSchemaVersion: Int!` on `QueryRoot`; the
only probe document is
`query ProviderExecutionTruthSchemaProbe { providerExecutionTruthSchemaVersion }`
and success is `data.providerExecutionTruthSchemaVersion == 1` with no
GraphQL errors. Handling is frozen:

| Probe result | App action |
|---|---|
| HTTP/network/auth failure | Surface existing daemon/auth error; do not replace for schema |
| GraphQL unknown-field validation error | Replace bundled daemon once, await readiness, retry probe once |
| Missing/null/non-integer/version other than 1, or data plus errors | Same one replacement/retry, then typed schema mismatch |
| Malformed response JSON | Typed daemon protocol error; no reduced query |
| Version 1 | Issue only the v1 run-detail document |

After one replacement attempt, every non-success renders
`Daemon schema mismatch` with retry/restart action; the app never loops replacement or falls
back to planned values as runtime truth.

The Swift DTO fields remain nullable for historical database rows and
pre-configuration executions returned by a daemon that advertises schema v1.
Missing required fields after a successful v1 probe are a contract violation,
not legacy compatibility.

## UI Truth and Formatting Contract

One `ProviderExecutionIdentityFormatter` owns visual text, Help text, and
accessibility text. It accepts planned, requested, accepted, execution status,
provider-configuration/evidence state, prompt-dispatch summary, and topology
association state. It never promotes a planned/requested value to accepted
truth, and an unresolved unknown prompt turn overrides ordinary execution
status copy.

### Codex state matrix

| State | Runtime truth | Operator copy |
|---|---|---|
| Pending | Frozen planned pair; no execution | `Planned: Codex - GPT-5.6 Terra - High` |
| Configuring | Requested pair present; accepted pair absent | `Configuring: Codex - GPT-5.6 Terra - High` |
| Cancelled during configuration | Accepted pair absent; configuration is terminal | `Cancelled before prompt: Codex - GPT-5.6 Terra - High - No prompt sent` |
| Configured / not started | Response-verified pair; prompt not attempted | `Configured: Codex - GPT-5.6 Terra - High - Prompt not started` |
| Start failed after configuration | Response-verified pair; execution failed while turn stayed `not_started` | `Start failed: Codex - GPT-5.6 Terra - High - No prompt sent` |
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
| Continuation pending | Original sent; P086 turn pending | `Using: Codex - GPT-5.6 Terra - High - Continuation starting` |
| Continuation sent | P086 turn durably sent | `Using: Codex - GPT-5.6 Terra - High - Continuation prompt sent` |
| Continuation unknown | P086 delivery ambiguous | `Continuation prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Configuration failure | Requested pair present; accepted pair absent | `Configuration failed: GPT-5.6 Terra - High - No prompt sent` |
| Legacy generic | Frozen requested identity; provider acceptance unavailable | Status prefix plus `Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| Retry/fallback | Latest execution for the same task occurrence | Codex uses that execution's accepted pair and dispatch state |

If `configured` lacks either accepted field, the Codex readback is internally
inconsistent. It renders `Runtime identity unavailable`, exposes diagnostic
Help text, and must be caught by the gate. It must not fall back to planned
truth.

`LEGACY_AMBIGUOUS` is a legal formatter input, not a schema failure. It renders
`Runtime identity unavailable - Multiple legacy executions`, Help text with the
bounded candidate count, and no accepted/requested runtime pair. Planned task
identity may remain on its separate planned line. It never selects one legacy
execution. A configured execution that fails before permit is likewise the
legal `Start failed after configuration` row above rather than an impossible
tuple.

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
| Continuation pending/sent | Status prefix plus `Continuation starting` or `Continuation prompt sent` and `Acceptance unverified` |
| Continuation unknown | `Continuation prompt delivery unknown: Claude - Opus - High - Do not retry automatically` |
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
The custom digest is lowercase SHA-256 of the exact UTF-8 raw model bytes with
domain prefix `chainworks.custom_model_label.v1` and one NUL separator; prefix
is the first 10 hex characters. Exact compact copy is
`Custom model sha256:<prefix>` followed by the ordinary effort segment when
present. Empty model is not hashed and remains absent. Fixtures include ASCII,
non-ASCII, leading/trailing whitespace, and two values sharing the first eight
hex characters to enforce the ten-character rule.

The checked-in formatter corpus is generated from the closed legal-state table,
not hand-picked examples. It includes every configuration state, execution
status (`pending`, `running`, `completed`, `failed`, `cancelled`), delivery
truth, provider class (exact Codex, legacy Codex, non-Codex), every known model
and effort, unknown non-empty values, and missing optional non-Codex segments.
For every legal tuple it freezes status prefix, `fullIdentity`,
`compactIdentity`, Help, copy, and accessibility strings; every illegal tuple
must return typed `identity_contract_invalid` rather than a best-effort label.
The gate proves each enum value appears in at least one legal golden and one
applicable mutation-negative. Visual, Help, copy, and accessibility outputs are
then byte-compared to the same formatter result.

Model and effort receive a dedicated identity line below the task/title line.
Overview and active-agent rows allow the full value to wrap. Topology uses
`ViewThatFits` with a two-line full identity followed by the bounded compact
identity and an `info.circle` button. That button opens a selectable popover
with the complete identity and a copy action. Status, attempts, stage, task, and
session diagnostics occupy a separate secondary line.

The info button is in keyboard focus order; Space or Return opens it and Escape
closes it. Opening moves focus to the selectable full identity, Tab reaches the
copy and close controls, and closing restores focus to the triggering
`presentationRowId`. If that row disappears while open, the popover dismisses,
clears stale copy state, and focuses the next surviving row in deterministic
presentation order (or the stage heading when none remains). Hosted tests cover
mouse, keyboard-only, VoiceOver labels, focus restoration, and row removal.

The hard-coded full-MVP map and sequential fallback are removed.
`StageTopologyLayoutBuilderV2` places every frozen transition graph
deterministically:

1. normalize nodes by frozen workflow ordinal then stage ID and edges by source,
   target, transition ordinal, and transition ID;
2. run deterministic Tarjan SCC decomposition; multi-node SCC members occupy
   one column on consecutive tracks and internal/back edges route around the
   outside of that SCC instead of affecting rank;
3. form the condensation DAG, split weakly connected components, and order
   disconnected components by their minimum node key;
4. assign each SCC column by longest path from a zero-indegree root, with roots
   in column zero and deterministic virtual nodes for edges spanning columns;
5. order each column by frozen node key, then perform exactly four alternating
   downward/upward median-neighbor sweeps, resolving equal medians by the prior
   order and node key;
6. assign the nearest free integer global track in that order, stack
   disconnected components with one empty separator track, and emit
   `column`, `trackStart`, and initial `trackSpan = 1` for each stage; and
7. route fork/merge edges through their virtual-node tracks, while cycle edges
   use a dedicated outer channel keyed by SCC and edge order.

No dictionary iteration, input array order, measured card height, or special
workflow ID participates in placement. Permuting identical graph input must
produce byte-equal placement.

Geometry constants are frozen in `StageTopologyMetricsV2`: column width 320 pt,
column gap 88 pt, minimum track height 96 pt, track gap 16 pt, ordinary branch
channel spacing 12 pt, outer-cycle inset 24 pt, and self-loop top inset 16 pt.
Coordinates snap to half-point boundaries. Every edge uses `midTrailing` as its
source port and `midLeading` as its target port. Forward fork/merge channels use
the inter-column midpoint plus `12 * parallel_edge_ordinal`, where the ordinal
comes from transition order. Back/cycle edges use the right outer SCC channel
`graphMaxX + 24 + 12 * channel_ordinal`, ordered by SCC key then transition ID.
A self-loop with ordinal `i` has exact orthogonal points
`[midTrailing, (maxX+24+12i, sourceY), (maxX+24+12i, minY-16-12i), (minX-24-12i, minY-16-12i), (minX-24-12i, targetY), midLeading]`.
Long-edge virtual nodes use the same
forward-channel rule. Parallel/self-loop fixtures freeze every point, port,
channel index, and half-point rounding.

Natural card height is computed from header chrome, metadata/transition rows,
occurrence count, and bounded two-line identity rows. Starting from the minimum
track height, layout processes stages in `(column, trackStart, stageId)` order
and increases that stage's global track by the exact point deficit. SCC groups
and future multi-track nodes distribute a deficit by integer points across
their span, assigning any remainder to ascending track IDs. The pass repeats
until every card fits; identical input yields identical track sizes and cards in
one column never overlap.

Each card frame is the sum of its global track heights and inter-track gaps.
Cards publish bounds through anchor preferences. Connector source and target
centers are the actual `midTrailing` and `midLeading` points of those frames;
orthogonal branch junctions use the midpoint of the inter-column gap. The same
global frames drive manually paired branches, hit testing, focus, popovers, and
accessibility. No connector computes y-position from a fixed card-height
constant.

Pure layout tests cover a fork, diamond merge, two-node cycle, self-loop,
long-edge virtual nodes, disconnected components, and shuffled input. Hosted
tests use the real full-MVP graph with mixed 1/2/5-occurrence cards and assert
non-overlap, stable tracks, actual-frame connector centers, bounded crossings,
and no fallback to the removed hard-coded map.

If the v2 topology projection is absent, schema-invalid, or carries an unknown
source/order value, the hosted view renders the exact state
`Topology unavailable` with action `Retry after daemon restart`; it renders neither the
old hard-coded map nor sequential guessed nodes. A hosted negative fixture
asserts this state, no connector/card accessibility elements, and successful
recovery when a valid projection replaces it.

The layout golden corpus crosses each graph shape with occurrence counts
`0, 1, 2, 5`, one-line/two-line/unknown-long identity rows, and transition
ordinals that do not match SQL input order. Every case freezes node/edge IDs,
columns, tracks, spans, edge channels, measured frames, connector endpoints,
focus order, and presentation-row IDs. Required invariants are: byte-equal
output under 100 deterministic input shuffles; no frame overlap; every edge
connects its declared transition ID; every connector endpoint equals the
published frame anchor; all rows are reachable in keyboard order; and no text
or popover clips at the minimum supported window width or 200% accessibility
text size.

Each occurrence row owns its accessibility label. Stage cards contain child
accessibility elements rather than combining and swallowing occurrence labels.
The row label is exactly
`<task name>. <fullIdentity>. Status: <status>. Attempts: <count>.` The info
control label is `Show full runtime identity for <task name>`, the copy control is `Copy full runtime identity for <task name>`,
and the close control is `Close runtime identity details`. Unknown delivery adds
the exact hint `Automatic retry is blocked.` to the row value; legacy ambiguity
adds `Multiple legacy executions; runtime identity is unavailable.` Tests assert
labels, values, hints, child order, and keyboard actions byte-for-byte.
`PresentationRowIdentity` is the sole encoder. It hashes domain-separated,
length-prefixed UTF-8 components and emits lowercase
`topology_row_v1:<sha256>` over exact components
`[run_plan_snapshot_sha256, state_id, source_kind, source_stable_id, compiled_task_id]`.
`run_plan_snapshot_sha256` is SHA-256 of duplicate-key-
rejected RFC 8785 `RunPlan` JSON with runtime IDs/timestamps absent, not the
workflow file hash. All four source kinds use the unified projection and the
pre-execution source ID defined above; no kind depends on StageExecution. The
value exists before a row is first rendered and never
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
| Owner receipt missing or owner-kind fields inconsistent | `ACP_PROVIDER_CONFIGURATION_OWNER_INVALID` | 0 |
| Provider process identity absent/ambiguous | owner quarantine; `ACP_PROVIDER_PROCESS_IDENTITY_UNVERIFIED` | 0 or unknown per turn state |
| Cancellation wins during configuration | `cancelled_before_prompt` | 0 |
| Original-owner reused generation evidence mismatch | close generation and negotiate fresh once | 0 on old session |
| P079/P086 generation evidence mismatch | fail owner; fresh fallback forbidden | 0 |
| P086 continuation lacks execution/occurrence/turn/work-item binding | `ACP_PROMPT_OWNER_INVALID` | 0 |
| P086 atomic admission insert fails | complete transaction rollback; no accepted response | 0 |
| Steward invocation lacks analysis/lane/agent/work-item binding | `ACP_PROMPT_OWNER_INVALID` | 0 |
| Dispatch permit loses to cancellation/ownership/epoch CAS | `ACP_PROMPT_DISPATCH_PREPARE_FAILED` | 0 |
| Initial prompt-turn CAS returns `Missing` | `ACP_PROMPT_TURN_MISSING`; owner blocked/failed | 0 |
| Bounded write deadline or cancellation wins after permit | `ACP_PROMPT_DISPATCH_UNKNOWN`; coordinator interrupts provider | unknown |
| Transport send/flush fails after dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Prompt-sent persistence fails after transport success | `ACP_PROMPT_DISPATCH_UNKNOWN` | sent or unknown |
| Startup finds stale dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Legacy InvokeAgent upgrade cannot derive owner identity | `invoke_agent_upgrade_identity_missing`; work item failed and owner blocked | 0 |
| Active v1 P079 `prompt_sent` migration | lease/turn `dispatch_unknown`; owner blocked | unknown |
| Active v1 P079 work-item owner absent/ambiguous | lease `legacy_unverified`; owner quarantine | unknown |
| Unsupported/malformed runtime receipt | typed receipt failure; no projection | preserve turn ledger |
| Topology execution lacks unambiguous occurrence identity | omit execution association; expose legacy ambiguity | unchanged |
| Schema v1 probe or selected-key contract fails | typed daemon schema mismatch; no reduced query | unchanged |
| Legacy generic frozen run | allowed as planned/unverified | shared ledger for each new attempt |

Configuration failures use `failure_phase = provider_configuration`, leave
accepted fields `null`, and may render the requested pair plus
`No prompt sent`. Dispatch failures use `failure_phase = prompt_dispatch`,
preserve the configured accepted pair, and never claim that no prompt was sent
after `dispatch_pending`. Unknown delivery atomically marks the owning work
item `Failed`; run-bound owners also block their stage/run, while Steward writes
its typed lane outcome and forbids automatic replay without inventing a run.
Missing accepted readback is never inferred from the host configuration or
planned catalog value.

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
| `workflow` + `domain` | Exact seven-profile matrix; per-profile frozen capabilities and effective fallback contract; validated Steward catalog; fresh generic/invalid rejection; legacy replay; sealed provenance union; exact nine-ID manifest; complete compiled-coordinate, condition, binding, dynamic-key, and occurrence vectors |
| `acp` fake provider + `engine` dispatch | Response-closed negotiation; generation-bound reuse; injected DB/runtime ports and permit-only API; 30-second whole-handshake and 10-second write deadlines; original, both P079 kinds, all three P086 modes, and both Steward lanes; complete CAS table; launch barrier/process identity; coordinator-only close; bidirectional fallback; Claude aliases unchanged |
| `db` + `engine` recovery | Executable staged preflight/finalizer and restart marker; owner receipt allocators/failures; pre-created terminal Steward lanes; owner-scoped lineage/process truth; prompt authority/quarantine; complete P079 budget/FK matrix; rebuilt P086 ledger and cartesian legacy classifier; sealed legacy envelopes; dynamic rebuild; closure-owned replay authorization |
| `daemon` composition | Real production construction of upgrade coordinator, durable authority, ACP manager, process-control port, and invalidation coordinator; no missing/default authority or direct handle path; consumers/listeners remain closed through finalization |
| `graphql-server` + `mcp-server` | Byte-equal complete `AppSchema::sdl()` plus `QueryRoot` probe matrix; closed enums/nullability; per-turn/total receipt links; GraphQL/JSON bijection; full MCP envelopes and structured-content parity; purpose-built redaction DTO with all principal classes; transition/topology source identity; planned values never populate accepted fields |
| Swift focused and hosted-view tests | Presence-aware DTO decoding; lockstep restart; complete state/ambiguity/start-failure matrices; formatter parity; unified immutable row key; frozen ordinal; exact connector/self-loop geometry; topology-unavailable state; long values; exact keyboard/focus/copy/accessibility contracts |

Swift proof is two independent invocations and result bundles:

1. `codex-model-truth-pure.xcresult` runs
   `ProviderExecutionIdentityFormatterTests`,
   `ProviderExecutionTruthDecodingTests`, and
   `StageTopologyLayoutBuilderV2Tests`.
2. `codex-model-truth-hosted.xcresult` runs
   `RunModelIdentityHostedTests` and `RunStageTopologyHostedTests` against the
   real run-detail views.

After each invocation, the gate parses that bundle independently with
`xcresulttool`, requires `totalTestCount > 0`, and requires every named class
and its frozen required test identifiers to appear with a passed result. Counts
are never summed across bundles; a populated pure bundle cannot hide a missing
hosted suite or vice versa.

The retained executable scenario matrix must include:

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
11. original success followed by each of
    `code_writer_completion_repair` and `output_contract_repair` proves
    independent durable turns, work-item-bound lease-v2 parity, one-time budget
    consumption, TTL behavior, and active v1 unique/ambiguous migration;
12. P086 admission atomically creates command journal, continuation, work item,
    turn, and reserved side effect; failure at each insert rolls back all and
    idempotent replay returns the same IDs;
13. live-handle, resurrection, and output-only P086 modes reserve distinct
    typed turns, require target execution/occurrence and ProcessContinuation
    item, preserve frozen mode across attachment conversion, mirror side-effect
    rows only after CAS, reject fresh-session fallback, and fail before I/O on
    wrong identity;
14. a Steward claim persists the running analysis and both lane owners before
    either provider call; `system_steward` and `steward_auditor` then use
    owner-scoped lineage/process/configuration/turn records without synthetic
    run/execution IDs, while a historical unowned running item is terminalized
    and quarantined without replay;
15. every initial/final combination of `Applied`, `AlreadyMatching`, `Conflict`,
    and `Missing` enforces owner/generation/request binding, including initial
    commit-ack loss, final commit-ack loss, zero-byte conflict, and missing-row
    quarantine;
16. a transport that never completes write/flush is cancelled within the fixed
    deadline, the invalidator never waits indefinitely on the gate, out-of-band
    process cleanup runs, and no public raw close/kill/cancel API bypass exists;
17. production API compilation proves `AcpSession`, handles, raw prompt/start/
    execute methods, and direct adapter fallback are unavailable outside `acp`;
    the sole manager send site consumes a one-use authority permit;
18. Claude, Gemini, Auggie, and Junie advance the shared prompt ledger while
    keeping provider-configuration truth non-applicable;
19. cancellation and both directions of cross-provider fallback preserve the
    state matrix and occurrence-association rules;
20. the launch barrier plus process-binding crash table is exercised at every
    boundary for original, both repairs, all continuations, and both Steward
    lanes; only identity-matched processes are reaped and no prompt is replayed;
21. cancellation during configuration, run/targeted-retry invalidation
    immediately before permit, and invalidation racing after permit are
    linearized by the generation gate; run-wide races additionally prove epoch
    fencing, while scoped races prove unrelated stages retain their epoch;
22. startup keeps consumers closed, applies every receipt/work-item/Steward/
    P079/P086 row in the persisted-work matrix, reconciles processes and
    unknown turns, and only then enables claims;
23. the generated replay manifest covers every enqueue/claim/requeue/prompt
    site and all selectors refuse unresolved, quarantined, stale, absent, or
    migration-pending authority;
24. all nine exact producer IDs, same-owner retry/fallback, replacement-stage
    retry, loop re-entry, dynamic idempotency/conflict, `legacy_flat`, and
    migration-only `LegacyInvokeEnvelopeV1` behavior match the complete
    component arrays and independently reproduced golden vectors; materialization
    and runtime binding changes never alter compiled identity;
25. canonical JSON tests include the RFC 8785 known answer, number/string
    vector, duplicate-key rejection, digest mismatch, pre-session failure,
    malformed receipt, all three link states, v1/v2 decode, and unsupported
    receipt version;
26. a generated legal/nullability matrix and mutation-negatives prove every
    closed enum and receipt/DTO combination;
27. fresh `AppSchema::sdl()` byte-matches the full checked-in snapshot with the
    actual Rust type names; schema probe action/error handling is executable;
28. one serializer fixture byte-matches `run://`, `report://`, `reports.get`,
    mediation, and generated run-report JSON; the explicit GraphQL bijection
    maps back to those bytes and Swift decodes the same domain value;
29. transition ordinal/ID and association-state fixtures prove shuffled-query
    parity, total latest-execution order, planned `NOT_STARTED`, legacy unique,
    and legacy ambiguous behavior; and
30. pure and hosted layout/formatter corpora prove all legal states, fork,
    merge, cycle, self-loop, long edge, disconnected graph, 100 input shuffles,
    mixed heights, minimum width, 200% text, keyboard popover focus restoration,
    and row removal.
31. staged migration interruption at every batch/finalizer boundary resumes from
    the durable marker, never opens a consumer on interim nullability, and
    rejects source-digest drift;
32. both owner kinds allocate configuration attempts concurrently with monotonic
    gaps, stale pointer CAS rejection, and `receipt_unavailable` cleanup/startup
    recovery when receipt persistence fails;
33. every P079 lease kind/state/budget/FK/classifier combination and every P086
    mode/status/old-ledger/attachment/receipt combination reaches exactly one
    frozen migration result without treating a pre-I/O write as sent;
34. system/auditor lanes cover valid report, missing report, no executor,
    configuration failure, unknown delivery, cancellation, and crash replay;
    both lanes and the analysis work item always become terminal;
35. Codex-to-non-Codex and non-Codex-to-Codex fallback persist target-derived
    effective contracts while preserving compiled/occurrence identity;
36. tool/resource JSON-RPC envelopes, `structuredContent`, GraphQL enum/field
    bijection, unlinked receipt summary, and Operator/Agent/Observer redaction
    fixtures round-trip through the domain DTO;
37. run-report materialization kill points converge to one authoritative file,
    journal state, artifact checksum, and ordered execution-truth array;
38. daemon production composition proves the actual authority/coordinator/process
    ports are installed before listeners and fails construction when any port is
    absent;
39. all four topology source kinds, pre-execution legacy rows, frozen/unverified
    ordinals, ambiguous association, post-configuration start failure, and
    topology-unavailable recovery execute in pure and hosted tests; and
40. condition canonical bytes, RunPlan/presentation hashes, ten-hex custom model
    labels, connector channels/ports/self-loop points, and exact accessibility
    labels match independent golden implementations.

The gate must fail independently when either Swift result bundle reports zero
tests or omits a required identifier. No network, daemon, live provider, or
remote UI host is required.

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
      per-profile capability map; every fallback persists its target-derived
      effective contract, and active Steward catalog loading uses the same
      validator instead of an unchecked parse.
- [ ] Frozen pre-change snapshots retain the old adapter path and remain
      planned/legacy-unverified rather than being upgraded or guessed.
- [ ] Codex exact negotiation consumes each returned option snapshot, verifies
      final model and effort, and exposes no prompt permit until durable
      acceptance succeeds; Claude alias matching remains unchanged.
- [ ] Requested and accepted truth is durable, nullable, versioned, owner-scoped,
      and tied to one stable task occurrence for run agents. Steward persists
      the running analysis and both lane owners before provider launch and uses
      no synthetic run/execution; both lanes always reach an explicit terminal
      or prerequisite-skipped state and old unowned running work is terminalized.
- [ ] AgentExecution and Steward owners expose exact monotonic configuration
      attempt allocators and receipt pointers. Receipt persistence failure has a
      separate zero-send failure row, deterministic process cleanup, and startup
      convergence without fabricating a receipt.
- [ ] Matching live-session generation evidence is projected before reuse;
      missing or mismatched evidence closes the old session with zero prompts
      on that handle and permits only one fully negotiated fresh fallback for
      an eligible original owner, never P079 or P086.
- [ ] Session lineage, provider session, and process binding support typed
      run-agent and Steward-lane owners. Launch intent/barrier and PID/start
      identity are durable before `session/new`, and crash recovery never
      signals an identity-ambiguous process.
- [ ] Production crates cannot construct a prompt permit or access raw
      `AcpSession`/handle/start/prompt/execute fallbacks; the sole manager send
      site consumes one authority-backed permit.
- [ ] `provider_prompt_turns`, not terminal runtime receipts or P079/P086 domain
      rows, is the sole dispatch authority. Original, both repair kinds, all
      three continuation modes, and both Steward agent prompts receive
      independent durable turns.
- [ ] Historical runtime receipts remain representable as `linked_v2`,
      `legacy_pre_prompt`, or `legacy_unverified`; only positive unique evidence
      creates a sent turn and new writes always carry a valid turn FK.
- [ ] P079 lease v2 state changes atomically with its turn, consumes budget once,
      binds a unique work item, covers both repair prompt kinds, has defined TTL
      behavior, and migrates absent/ambiguous v1 evidence to quarantine/unknown.
- [ ] P086 admission atomically commits command, continuation, work item, turn,
      and reserved side effect. All three modes carry the real execution and
      occurrence, mode survives attachment conversion, and no side effect can
      claim sent before the final CAS. The rebuilt ledger accepts every v2 state
      and maps all old pre-I/O `committed/prompt_sent` evidence to unknown unless
      correlated post-I/O proof exists.
- [ ] Dispatch and cancellation share the invalidation coordinator and one
      generation gate; run-wide invalidation additionally uses the durable
      epoch. A cancelled, terminal, stale-owner, or inactive-generation prompt
      writes zero bytes when invalidation wins.
- [ ] Transport write/flush is cancellation-aware and bounded to ten seconds;
      invalidation can interrupt the supervised process out of band, cannot
      deadlock on the gate, and is the only public close/kill/cancel path.
- [ ] The complete initial/final four-result CAS table covers commit-ack loss,
      zero-byte conflict, missing-row quarantine, and final unknown settlement.
- [ ] The complete launch/configure/permit/write/final/receipt crash table
      converges to one process owner and deterministic settlement for every
      prompt kind without duplicate provider I/O.
- [ ] Unknown delivery marks the owning item failed, blocks a run-bound owner or
      records the typed Steward lane outcome, and is excluded by every automatic
      retry, continuation, fallback, claim, and startup-requeue selector.
- [ ] Startup keeps every consumer closed until the complete legacy row matrix,
      dynamic rebuild, receipt states, Steward/P079/P086 migration, process
      reconciliation, and unknown quarantine finish; no old running item can be
      replayed first.
- [ ] SQLx staging, Rust backfill, final constraint rebuild, durable restart
      marker, write fence, and failed-serve behavior form one executable
      preflight protocol; replay authorization cannot escape its owning closure
      or transaction.
- [ ] A generated `ReplaySelectorIdV1` manifest covers every prompt-capable
      enqueue/claim/requeue/send site; new unclassified production paths fail
      the structural gate.
- [ ] Every one of the nine production `InvokeAgent` source classes delegates
      to typed enqueue/claim validation; `legacy_flat` is explicit, same-owner
      retry/fallback preserves identity, and a new stage execution recomputes it.
- [ ] `LegacyInvokeEnvelopeV1` is migration-only and absent from
      `ProducerIdV1`; compiled coordinates exclude owner, materialization,
      provider fallback, and loop-instance data, and complete golden arrays are
      independently reproducible.
- [ ] Dynamic materialization persists compiled-task, occurrence, and work-item
      identity atomically, migrates the historically misnamed column without
      treating a work-item ID as an execution ID, and fails closed on conflict.
- [ ] Identity codecs and receipt schemas freeze exact fields and bytes; golden
      SHA-256 vectors, duplicate-key rejection, RFC 8785 known answers, verified
      digests, and the v1/v2 decoder matrix are executable fixtures.
- [ ] The app proves schema v1 before the new GraphQL document, performs at most
      one bundled-daemon replacement/retry, and fails visibly on persistent
      mismatch.
- [ ] GraphQL and Swift distinguish planned, configuring, configured but not
      started, prompt sent, delivery unknown, failure, cancellation, and legacy
      states without treating non-Codex null configuration as an error.
- [ ] GraphQL, mediation attempts, MCP execution truth, and run reports expose
      one versioned sanitized truth DTO through the exact JSON-RPC/MCP envelopes;
      canonical JSON parity, GraphQL naming/enum bijection, per-turn plus total
      receipt-link truth, principal-class redaction, complete generated SDL,
      historical empty-list semantics, and Swift omitted-versus-null behavior
      match checked-in fixtures.
- [ ] The engine, not the lead agent, materializes authoritative execution truth
      into `run_report` through a crash-consistent journal/rename/checksum flow;
      daemon-owned composition installs the real authority and process-control
      ports before listeners or consumers open.
- [ ] Codex/non-Codex fallback in either direction never inherits incompatible
      accepted truth and remains tied to the original occurrence.
- [ ] Full model/effort identity is identical across visual, Help, popover,
      copy, and accessibility output; bounded compact text never abbreviates a
      known Sol/Terra/Luna pair, and keyboard focus survives close/removal.
- [ ] Deterministic graph placement replaces the hard-coded/sequential maps and
      proves all four source kinds, pre-execution stable IDs, frozen ordinals,
      exact condition/presentation bytes, fork, merge, cycle, self-loop channels,
      long-edge, disconnected, shuffled, legacy ambiguity, start-failure and
      topology-unavailable states, exact accessibility labels, and real
      mixed-height topology with actual-frame connector centers.
- [ ] `./scripts/test-gate.sh codex-model-truth` passes with nonzero Rust and
      independently nonzero required pure and hosted Swift test execution.
