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
source-scoped occurrence sequence,
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
`run`, `run_after_approval`, `owner`, `mediation`, `escalation`, or `legacy_flat` and therefore
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
| `orchestrator.p058_escalation_retry` | compile | `[workflow_snapshot_sha256, origin_state_id, "escalation", "p058_escalation", frozen_tier_ordinal, "0", "0", escalation_tier_id, frozen_tier_binding_sha256]` |
| `p058_deadline_resume.operator_resume` | preserve/compile | Preserve the escalation coordinate only when every identity-bound field is unchanged; otherwise compile the selected frozen tier coordinate above |

`block_kind` is one of `run.sequence`, `run.parallel`, `run.then`,
`run.dynamic_parallel`, `owner`, `lead_conflict_mediation`, `p058_escalation`, or
`legacy_flat`.
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

P058 escalation never preserves the source task's compiled identity while
changing agent, prompt template, permissions, output contract, provider, model,
or effort. Each frozen escalation-policy tier has its own binding and coordinate
above; tier ordinal is its order in the frozen policy, and tier ID is unique in
that policy. Source compiled-task/occurrence IDs remain correlation fields, not
hash substitutions. Deadline resume may preserve an escalation compiled-task ID
only for the same tier binding. A bounded operator instruction is an occurrence
input artifact and does not mutate the frozen prompt template; selecting another
tier or changing any identity-bound field compiles that tier's distinct ID and
a replacement-owner occurrence.

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
The gate reproduces both with the Rust implementation and independent stdlib-
only `scripts/reference/provider_truth_vectors.py`, which imports no production
crate, generated fixture, or Swift code. Swift independently decodes/re-encodes
the committed vectors in
`Chainworks ForgeTests/IndependentProviderTruthVectorTests.swift`; it does not
call the production formatter/hash helper. Fixtures include every producer, the
complete component arrays, reordered input, empty component, non-ASCII byte,
malformed length, unknown producer, `run` versus `run_after_approval`, every
sequence/parallel/then lane position, owner/mediation/legacy coordinates,
dynamic selector replans with same agent/different plan or binding, and
forbidden dynamic-component cases. A pairwise collision harness requires all
logically distinct coordinates to produce distinct IDs. Mutation checks change
one tag, component byte, component order, u32 endianness, ordinal, nullable tag,
or timestamp fractional digit and require a digest/parse mismatch.

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
new ID, so attempts from different owner scopes cannot be merged. In the same
owner-creation transaction, `TopologyOccurrenceAllocator` assigns non-negative
`occurrence_sequence`, monotonically increasing per
`(run_id, source_stable_id)`, and stores it in the envelope, execution, and
topology projection. An idempotent replay returns the existing sequence; it
never allocates a second value for the same task-occurrence ID. Pending
static/owner topology exposes `compiled_task_id`; its `task_occurrence_id` and
sequence remain `null` until a stage execution exists.

One domain projection, `RunStageTopologyOccurrenceV2`, represents every row.
Its `source_kind` is closed to `static_compiled`, `owner_compiled`,
`dynamic_materialized`, or `legacy_flat`; it also carries a non-null
`source_stable_id`, `compiled_task_id`, frozen stage/task ordinals, planned
binding, nullable occurrence ID/sequence/execution truth, and association state.
`TopologySourceStableIdV1` uses prefix `topology_source_v1:` and common-codec
domain `chainworks.topology_source.v1`. Static/owner components are exactly
`[run_plan_identity_marker, state_id, source_kind, compiled_task_id]`; dynamic
components add `dynamic_task_key` before compiled-task ID; legacy components are
`[run_id, workflow_identity_marker, state_id, source_kind, frozen_task_ordinal, agent_id, compiled_task_id]`. The compiler persists static
and owner IDs in RunPlan, materialization persists the dynamic ID before its
work item is visible, and migration computes the legacy ID without a
StageExecution. `run_plan_identity_marker` is the frozen RunPlan snapshot
SHA-256 when present, otherwise the persisted
`legacy_workflow_v0:<workflow_id>` marker. Consequently every planned, pending,
dynamic, and legacy source has stable identity before execution. Readback emits
one planned row only while the source has no occurrence; once occurrences
exist, it emits one row per durable task-occurrence ID and does not collapse
loop/replacement history back into the source row.

The compiler persists `frozen_workflow_ordinal` on every topology node and
`frozen_task_ordinal` on every occurrence. State ordinal is source YAML state
sequence order after the parser's preserved mapping order; the compiler fails
if that order is unavailable rather than deriving it from a hash map. Migration
uses the persisted historical stage order when unique and otherwise orders by
`stage_id` while marking `legacy_order_unverified`; the UI surfaces that state
and never calls it frozen truth.

Overview and every non-graph occurrence list normalize source rows by
`frozen_workflow_ordinal`, `frozen_task_ordinal`, `source_kind`, then
`source_stable_id`. Within a source, occurrence rows order by
`occurrence_sequence DESC, task_occurrence_id ASC`. `current` is the greatest
sequence whose owner stage is non-terminal, or the greatest sequence overall
when all are terminal; every lower sequence is `previous`. More than one
non-terminal occurrence is surfaced as an execution invariant violation but the
same greatest-sequence rule remains deterministic. Execution attempts inside
one occurrence use the canonical timestamp/UUID order below. No API array order,
SQL row order, agent ID, or completion arrival order controls presentation.

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

P086 provider-session resurrection never copies acceptance from the source
daemon generation into a newly attached generation. Attach first establishes
the target-bound provider-session and process identity with zero prompt bytes.
For `codex_exact_pair_v1`, the manager then reserves the continuation owner's
single active configuration attempt and re-runs the model and effort
set/readback sequence against that attached session, without `session/new`.
Only response-verified equality may create a new generation acceptance and
owner receipt with `acceptance_source = attached_session_reverification`.
The attach receipt, active attempt, new generation, process binding, acceptance,
and continuation turn form one authority tuple before a permit. If the provider
cannot re-read and confirm both options, attachment fails zero-send; old
generation evidence is never transferred by ID or digest.

Legacy v0 generations may be reused only by a
`legacy_best_effort_v0` execution. A `codex_exact_pair_v1` request never
inherits legacy-unverified generation evidence.

## Durable Runtime Truth

The next SQLite migration adds these columns to `agent_executions`:

| Column | Meaning |
|---|---|
| `task_occurrence_id` | Stable occurrence shared only within one owner scope |
| `task_occurrence_sequence` | Monotonic source-scoped presentation sequence allocated with the occurrence |
| `requested_model` / `requested_effort` | Canonical pair requested for this execution |
| `accepted_model` / `accepted_effort` | Canonical response-verified pair; otherwise `null` |
| `accepted_model_wire_value` / `accepted_effort_wire_value` | Exact provider option values whose `currentValue` was verified |
| `provider_configuration_state` | `configuring`, `configured`, `failed_before_prompt`, `cancelled_before_prompt`, or `legacy_unverified`; `null` for non-Codex |
| `provider_configuration_verified_at` | Complete-pair verification time; otherwise `null` |
| `provider_configuration_receipt_json` / `provider_configuration_receipt_sha256` | Bounded projection of the authoritative owner-scoped receipt and its verified digest |
| `acceptance_source` | `fresh_negotiation`, `reused_session_generation`, or `attached_session_reverification`; otherwise `null` |
| `configuration_evidence_state` | Non-null `pending`, `receipt_available`, `receipt_unavailable`, `not_applicable`, or `legacy_unverified` |
| `next_configuration_attempt_index` | Non-null monotonic allocator, initialized to `0` |
| `active_configuration_attempt_index` / `active_configuration_generation_id` | Nullable pair reserved atomically before provider launch; at most one pair exists per owner |
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
- rebuilds `agent_work_continuations` so its persisted mode CHECK accepts all
  three frozen P086 modes and its resurrection/worker/release columns obey the
  classifier below;
- creates `output_contract_repair_operations_v1` so one logical P079 budget
  consumption can own multiple bounded zero-send infrastructure attempts;
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
`active_configuration_attempt_index`, nullable
`active_configuration_generation_id`, nullable
`current_provider_configuration_receipt_id` FK, and non-null
`configuration_evidence_state` under the same rules as an execution. It also
stores `max_zero_send_retries INTEGER NOT NULL DEFAULT 1`,
`zero_send_retries_consumed INTEGER NOT NULL DEFAULT 0`, and
`next_prompt_turn_index INTEGER NOT NULL DEFAULT 0`, with
`0 <= zero_send_retries_consumed <= max_zero_send_retries`. The agent check is
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
the analysis and single StewardAnalysis work item settle atomically through the
following sole terminal reduction. No other section or implementation path may
reinterpret `prompt_delivery_unknown`. These are the existing durable
`StewardAnalysisStatus` values; this change introduces no additional status.

| System lane | Auditor lane | Analysis / work item |
|---|---|---|
| `completed` with validated health report | `completed` | `Completed` / `Completed` |
| `completed` with validated health report | `configuration_failed`, `failed`, or `prompt_delivery_unknown` | `Inconclusive` / `Completed` |
| `completed` without a valid health report | `prerequisite_skipped(system_health_report_unavailable)` | `Failed` / `Failed` |
| `configuration_failed`, `failed`, or `prompt_delivery_unknown` | `prerequisite_skipped(system_health_report_unavailable)` | `Failed` / `Failed` |
| `prerequisite_skipped(agent_executor_unavailable)` | same | `Failed` / `Failed` |
| either lane `legacy_unverified`, counterpart terminal or skipped | settle/quarantine the unverified lane without replay | `Failed` / `Failed` |
| any state when cancellation or replacement wins | settle each non-terminal lane as `cancelled_before_prompt` when its turn is `not_started`, otherwise `prompt_delivery_unknown` | `Superseded` / `Cancelled` |

Any pair not matching one row is `steward_lane_reduction_invalid`, leaves the
work item failed, and fails startup/readback verification. No terminal or
skipped lane is eligible for startup requeue. Cancellation, system failure,
auditor failure, missing output, and crash after either lane settlement each
have an executable row fixture.

The only automatic Steward replay is a zero-send infrastructure retry. A CAS
may increment `zero_send_retries_consumed` and return the same lane to
`reserved` only when its authoritative turn is `not_started`, no receipt or
side effect is unverified, no process can have written prompt bytes, the lane
is not cancelled/superseded, and the increment does not exceed
`max_zero_send_retries = 1`. The retry allocates a new configuration attempt,
generation, process binding, and prompt turn; it never reuses evidence. Attempt
`max + 1`, delivery unknown, or any positive/ambiguous I/O settles through the
table above and cannot requeue. Crash fixtures stop before and after the retry
counter CAS, new-turn insert, launch barrier, and terminal settlement and prove
one retry at most.

`session_lineages` gains non-null `lineage_owner_kind` and
`lineage_owner_id` plus nullable `continuation_id` and `steward_lane_id` FKs.
Kind is `run_agent`, `p086_continuation`, or `steward_agent_lane`. For
`run_agent`, `run_id` is non-null and equals `lineage_owner_id`; for P086,
`run_id`, target execution, and occurrence are non-null, `continuation_id`
equals `lineage_owner_id`, and the lineage belongs to that continuation rather
than the target execution; for Steward,
`run_id` is null, `steward_lane_id` is non-null and equals
`lineage_owner_id`, and `agent_id` equals that lane's agent. Existing rows
backfill `run_agent` with null continuation/lane FKs.
`provider_sessions` similarly gains owner kind `agent_execution`,
`p086_continuation`, or `steward_agent_lane`, owner ID, nullable `run_id`, and
nullable continuation/lane FKs; its CHECK requires the matching execution/run
tuple, the matching continuation/target tuple, or the matching non-run lane
tuple. No synthetic RunId or AgentExecution is permitted.

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
`invoke_agent_work_item`, `p017_mediation`, `p079_lease`, `p086_continuation`,
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
nullable `continuation_id` and `steward_lane_id` FKs, accepted pair, wire pair, source digest,
verified time, and failure code follow
the nullability rules of `ProviderConfigurationReceiptV1`. Owner kind is
`agent_execution`, `p086_continuation`, or `steward_agent_lane`. A database CHECK requires both
execution and occurrence for `agent_execution`, requires the owner ID to equal
the execution ID with null continuation/lane FKs; requires execution,
occurrence, and continuation FK for `p086_continuation`, requires owner ID to
equal the continuation ID, and requires a null lane FK; and requires both
execution and occurrence null plus a
non-null lane FK equal to the owner ID for `steward_agent_lane`.
`(configuration_owner_kind, configuration_owner_id, configuration_attempt_index)`
is unique. The owner row stores the next index, nullable active attempt/generation
pair, and current receipt ID. A permitted zero-send renegotiation appends a new
attempt and atomically moves that pointer; it never overwrites prior evidence.
A configured run-agent insert writes the receipt, pointer, and exact
`agent_executions` projection in one transaction; mismatch on read is evidence
corruption. A configured P086 attachment writes the receipt and its continuation
pointer without modifying the target execution's active configuration attempt
or accepted pair. A configured Codex Steward invocation writes the receipt and
lane pointer because no synthetic `AgentExecution` exists.

`agent_work_continuations` therefore owns non-null
`next_configuration_attempt_index INTEGER NOT NULL DEFAULT 0`, nullable
`active_configuration_attempt_index`, nullable
`active_configuration_generation_id`, nullable
`current_provider_configuration_receipt_id`, and non-null
`configuration_evidence_state`. P086 attach/reverification reserves and settles
this tuple; it must not borrow the target execution's allocator, active pair, or
receipt pointer. The continuation's requested pair is copied from its frozen
effective contract, while its accepted pair exists only in the continuation-
owned receipt/generation. A target execution and its continuation may therefore
retain different generation-scoped acceptance without either overwriting the
other.

Allocation is single-flight. In one owner-row CAS inside `BEGIN IMMEDIATE`, the
caller pre-generates generation ID `g`, requires both active fields null, reads
`next_configuration_attempt_index = n`, inserts generation `g` in pre-session
state with the same lineage/owner/attempt, and writes next index `n + 1` plus
active pair `(n, g)`. The rebuilt generation table permits null provider-session
and process fields only in this pre-session state. A second caller receives `configuration_attempt_active`;
it does not skip to `n + 1` or launch another process. The generation, launch
intent, and eventual process binding all carry `(owner, n, g)`. Receipt or
failure settlement requires that exact active pair. Success inserts
`(owner, n)`, moves the current receipt pointer, and clears the active pair in
one transaction. Failure appends the failure row and clears it only after
identity-safe cleanup is terminal; ambiguous cleanup leaves the pair and owner
quarantined for startup. Gaps from a transaction that committed an allocation
but crashed before launch remain valid and are never reused.

Reservation is legal only while every owner turn is `not_started` and any prior
generation has been identity-safely closed. It atomically clears the prior
current-receipt pointer and marks that receipt superseded-for-dispatch before
installing `(n, g)`; a failed new attempt can never reactivate old acceptance.
Initial dispatch requires active fields null, so it cannot race an in-progress
renegotiation.

The dispatch CAS joins the owner's current receipt to its exact attempt and
generation, then joins that generation to one running, identity-verified
process binding. The resulting private permit is bound to owner, attempt,
receipt digest, generation, provider session, process-binding ID/start identity,
effective contract, work item, and prompt turn. A stale receipt pointer,
different generation/process, or another attempt is not transferable authority.

Historical rows backfill next index `0`, both active fields null, and a null
pointer; rows for which migration creates a receipt use one greater than the
greatest inserted attempt and point at that row. The final schema rebuild
installs owner active-generation and receipt FKs after both target tables exist.
Tests race two allocators for each owner kind, crash every reservation/launch/
settlement boundary, and prove exactly one launched generation, monotonic gaps,
pointer CAS, stale-attempt rejection, and one dispatch-capable receipt.

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
occurrence, captured run epoch, `mediation_record_id` FK,
`escalation_ledger_id` FK, and `steward_lane_id` FK; contract version;
`dispatch_state`;
start/sent/unknown timestamps; typed failure code; and created/updated
timestamps. Foreign keys bind execution when present and always bind the work
item. Owner kind is `invoke_agent`, `p017_mediation`, `p058_escalation`,
`p079_repair`, `p086_continuation`, or `steward_agent_lane`. A CHECK requires
execution, occurrence, and run epoch for the first five with null lane FK;
`p017_mediation` additionally requires a mediation FK and owner ID equal to the
mediation-owned AgentExecution ID and a null escalation FK;
`p058_escalation` requires a non-null escalation FK matching the execution's
ledger and null mediation FK; the other run owners require both special-owner
FKs null. Steward requires execution, occurrence, epoch, mediation, and
escalation FKs null plus a lane FK equal to owner ID. The exact row-level SQL
CHECK for `steward_agent_lane` requires all execution, occurrence, run-epoch,
mediation, and escalation columns to be null and
`steward_lane_id = prompt_owner_id`. SQLite
`BEFORE INSERT/UPDATE` triggers (not a cross-table CHECK) additionally require
the referenced work item to be `steward_analysis` with `run_id IS NULL` and
`stage_id IS NULL`, and require the lane's analysis, agent, generation, lineage,
and work-item IDs to match. Partial unique indexes
enforce `(agent_execution_id, turn_index)` when an execution exists and
`(prompt_owner_kind, prompt_owner_id, turn_index)` otherwise. Owner-specific
admission rules, not a kind-only unique index, enforce one P086 send while
allowing a bounded Steward zero-send retry. New state is non-null and
checked to `not_started`, `dispatch_pending`, `prompt_sent`, or
`dispatch_unknown`; legacy ambiguity is represented as `dispatch_unknown`, not
SQL null. No receipt JSON or terminal provider status lives in this table.

The exact Steward branch included in the row CHECK is:

```sql
prompt_owner_kind <> 'steward_agent_lane' OR (
  agent_execution_id IS NULL AND
  task_occurrence_id IS NULL AND
  captured_run_epoch IS NULL AND
  mediation_record_id IS NULL AND
  escalation_ledger_id IS NULL AND
  steward_lane_id = prompt_owner_id
)
```

`PromptTurnAllocator::reserve_tx` is the only constructor. Claim/start inserts
`original/0`, sets `next_prompt_turn_index = 1`, and creates the execution in one
transaction for ordinary, P017 mediation-owned, and P058 escalation-owned
AgentExecutions; P017 also binds the mediation record and P058 binds the
escalation ledger. Every later run-bound prompt atomically reads/increments that
counter, so P079 and P086 cannot both claim index 1. A Steward invocation uses
the durable `steward_agent_lanes.id`, reads/increments that lane's
`next_prompt_turn_index`, inserts `steward_analysis/<allocated index>`, and never
allocates from an `AgentExecution`. Exact prompt
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

The domain enum and every SQL parser add the exact
`WorkItemKind::OutputContractRepair` / `output_contract_repair` value. A repair
attempt must use this kind; `InvokeAgent` remains reserved for ordinary and
fallback child executions. Claim, status transition, startup selector, metrics,
and MCP/GraphQL projections handle the new kind explicitly. Unknown kinds still
fail closed. A retained enum/SQL round-trip gate prevents a repair row from
being serialized or claimed as `invoke_agent`.

An append-only migration rebuilds `output_contract_repair_leases` as
`output_contract_repair_leases_v2`. Its state check is `reserved`,
`dispatch_pending`, `prompt_sent`, `dispatch_unknown`, or `settled`; it adds
`repair_prompt_kind`, nullable `work_item_id`, non-null
`work_item_link_state`, nullable `prompt_turn_id`, `dispatch_started_at`,
`prompt_sent_at`, and `dispatch_unknown_at`. `repair_prompt_kind` is
`code_writer_completion_repair` or `output_contract_repair` for a repair lease
and null for a fallback lease. `work_item_link_state` is `linked_v2` or
`legacy_unverified`. For every new repair lease, `linked_v2` requires a non-null
`output_contract_repair` work-item FK and its P079-owned prompt-turn FK. For every new fallback
lease, it requires the fallback InvokeAgent work-item FK and that child's
`original` prompt-turn FK; the lease projects but does not own that turn. Only
migrated terminal/unverified rows may have either FK null.
`dispatch_committed_at` remains a deprecated readback alias and equals
`prompt_sent_at` only for v2 rows. Domain enums, repository parsers, indexes,
TTL sweeps, and reference schemas change in the same release.

`output_contract_repair_operations_v1` owns logical operation ID, parent
execution/occurrence, selected repair/fallback kind, one permanently consumed
semantic budget, `max_infrastructure_attempts INTEGER NOT NULL DEFAULT 2`, next
infrastructure-attempt index, and terminal result. Each
lease is one attempt and adds non-null operation ID plus attempt index, unique
together. Creating the operation consumes exactly one selected budget; a repair
sets only `repair_budget_consumed`, a fallback only
`fallback_budget_consumed`, and the opposite flag remains false.

Repair admission creates the operation, typed `OutputContractRepair` work item,
`not_started` turn, and attempt-0 lease atomically. Fallback uses
`claim_fallback_with_lease_tx`: it pre-generates child execution and turn IDs,
then in one transaction validates the parent, creates/starts the child
AgentExecution and fallback InvokeAgent item, inserts its `original/0` turn,
creates the logical fallback operation, consumes fallback budget, and inserts
attempt-0 lease. A fallback lease can therefore never reference a child turn
whose AgentExecution does not exist. Failure at any insert rolls back all rows.

Permit moves the attempt lease/turn to
`dispatch_pending` and sets only `dispatch_started_at`; successful flush plus
final CAS moves both to `prompt_sent` and sets `prompt_sent_at`; ambiguous
delivery moves both to `dispatch_unknown`. Terminal output settlement moves the
attempt and operation to `settled` without changing canonical turn truth.
Budget consumption is never refunded. A TTL-expired `reserved` attempt with
turn still `not_started` settles only that attempt `deadline_exceeded`; the
explicit two-attempt infrastructure allowance may atomically allocate attempt
`n + 1` under the same operation without consuming another logical budget.
For repair this creates a fresh repair item/turn; for fallback it uses the same
atomic admission routine to create a fresh child AgentExecution, InvokeAgent
item, and original turn. Prior attempt/execution rows remain terminal evidence
and are never reused.
Pending, sent-without-result, or unknown expiry settles the operation with
`delivery_unknown`, records `ttl_expired_dispatch_pending` or
`ttl_expired_prompt_sent`, and blocks replay.

The state/nullability/budget matrix is normative:

| Lease row | Work item / turn | Budget truth | Migration/result |
|---|---|---|---|
| New repair, any active state | `OutputContractRepair` item and P079 turn non-null | Operation repair true, fallback false | Attempt mirrors its turn atomically |
| New fallback, any active state | Existing child execution, fallback InvokeAgent item, and original turn non-null | Operation repair false, fallback true | Attempt mirrors the child original turn |
| Zero-send expired attempt | Prior item/turn terminal and provably `not_started`; new attempt gets new item/turn | Same operation budget, no second consumption | Bounded next attempt or terminal deadline result |
| Migrated terminal v1 | Nullable, `legacy_unverified` when not uniquely provable | Preserve already consumed kind; never consume the opposite kind | `settled`, never replayed |
| Active v1 `reserved`, unique owner/kind, budget false | Create/bind one `not_started` turn in the same transaction | Consume selected budget once; if unavailable settle `budget_exhausted` | Remain `reserved` only after both commits |
| Active v1 `reserved`, unique owner/kind, budget true | Create/bind one `not_started` turn | Validate exactly the matching kind flag | Remain `reserved` |
| Active v1 `prompt_sent` or old send-side effect | Bind a turn only with unique owner; otherwise null plus quarantine | Atomically adopt one historical attempt for the matching budget when false | Always `dispatch_unknown`, never proven sent |
| Any active row with ambiguous owner, kind, turn, or contradictory budget flags | Nullable legacy links | Preserve flags; consume nothing new | `dispatch_unknown` plus quarantine |

The migration also rebuilds `output_contract_repair_fallback_parent_links` and
the repair-attempt parent link. Both carry non-null `operation_id` and
`attempt_index`; their parent identity is unique only on
`(operation_id, attempt_index)`, never globally on `repair_event_id` or
`parent_failed_agent_execution_id`. A fallback child execution ID remains
globally unique. Each new attempt therefore points to its own work item, turn,
lease, and child when applicable while preserving one logical parent/budget.
Existing migration-095 links become attempt 0 under a newly created operation,
preserving link ID, event ID, parent/child IDs, policy hashes, principal
attribution, and timestamps byte-for-byte. Duplicate or contradictory legacy
links become `legacy_unverified` plus owner quarantine; the upgrader never drops
one to satisfy the new key.

Migration maps terminal v1 leases to `settled` without inventing prompt truth.
It creates one logical operation plus attempt-0 row per distinct v1 lease key,
moves the preserved budget flags to the operation, and never merges two legacy
leases merely because they share a parent execution.
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

`P079MigrationEligibilityReducerV1` is exhaustive and runs before any active
row can remain/re-enter `reserved`. Its inputs are lease/result/TTL; repair
event and source-generation claim; run, stage, parent execution, and work-item
status; cancellation/supersession/replacement markers; pending approval;
principal existence, class, scope, disablement, and revocation; selected budget
and prior operation/attempt rows; prompt turn, receipt, quarantine, and provider
process evidence. Only an unexpired, non-cancelled, non-superseded, uniquely
owned row with still-authorized principal, eligible parent scope, available or
already-matching logical budget, zero-send proof, and no active quarantine may
remain `reserved`. Cancellation/supersession settles cancelled; TTL settles
deadline-exceeded; lost authorization settles policy-denied; exhausted budget
settles budget-exhausted; possible delivery becomes dispatch-unknown; every
missing/contradictory identity becomes legacy-unverified plus quarantine.
Generated cartesian fixtures cover every reducer input, prove one result, and
prove a zero-send infrastructure retry retains the same operation/budget while
using a new attempt, work item, and turn.

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

The inserted continuation initializes its own configuration allocator to zero,
active attempt/generation and current receipt to null, and evidence to `pending`
for exact Codex resurrection or `not_applicable` for a provider without this
contract. Live-handle continuation may project its already validated active
generation only after an owner-matching receipt is copied into a new
continuation-owned attempt; it never points directly at the target execution's
receipt. These columns and their FKs are part of the same five-record admission
transaction and idempotent replay comparison.

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

The migration also rebuilds `agent_work_continuations`; it does not attempt to
insert `output_only_recovery` through the old table CHECK. The new mode CHECK is
exactly `live_handle_continuation | provider_session_resurrection | output_only_recovery`.
Resurrection phase v2 is the closed nullable enum `admitted`, `launching`,
`launched`, `attaching`, `attached_unprompted`, `configuration_reverified`,
`prompt_dispatch_pending`, `prompting`, `prompt_sent`, `settling`, `cancelling`,
`completed`, or `failed_closed`. It deliberately contains every existing P086 phase;
the migration never aliases an old phase to an unrelated terminal bucket.
Non-resurrection rows require it null except output-only after its one-way
attachment conversion. Worker ID,
provider-process binding, heartbeat, lease release, attach receipt, and terminal
idempotency-key FKs have state-dependent nullability enforced by the final
rebuild.

`P086LegacyDispatchClassifierV1` evaluates the complete cartesian product of
continuation mode/status, ProcessContinuation status, old provider-send row
absence or `planned|started|committed|released|failed`, attach-receipt
absence/failure/success, resurrection phase, supervised-worker identity,
provider-process identity, heartbeat freshness, lease-release state, terminal
command/idempotency evidence, and terminal runtime-receipt evidence. Reduction
precedence is closed: positive correlated post-I/O evidence, possible delivery,
terminal no-replay, provable zero-send, then invalid/contradictory quarantine.

| Historical evidence | v2 result |
|---|---|
| No send row, continuation accepted/queued, valid bound work item, no positive I/O evidence | `reserved` with a newly allocated `not_started` turn |
| Old `planned`, otherwise same unique owner tuple | `reserved`; old row alone is zero-send evidence |
| Old `started` or `committed`, continuation `prompt_sent`, or any contradictory active combination without positive post-I/O evidence | `dispatch_unknown`, owner quarantine, no replay |
| Old `released`, release durably precedes any I/O boundary, phase is no later than `attached_unprompted` or `configuration_reverified`, and identity-matched process absence is proved | `failed` with `not_started` turn; zero-send retry follows only the continuation policy |
| Old `released` with missing ordering, phase `prompting` or later, live/ambiguous process identity, or any positive I/O evidence | `dispatch_unknown`, owner quarantine, no replay |
| Unique owner plus runtime receipt with positive post-I/O prompt timestamp and matching provider session/request fingerprint | `prompt_sent`, linked turn, no replay |
| Terminal continuation with no active replay path | Preserve terminal status; map ledger only for readback, never enqueue |
| Missing/duplicate owner, mismatched attachment, failed send row, or contradictory terminal evidence | `failed` or `dispatch_unknown` according to possible bytes, quarantine, no replay |

An active attached/resurrection row may remain zero-send `reserved` only when
its ProcessContinuation item is uniquely bound, worker lease and heartbeat are
current, process start identity matches the attach receipt, release is absent,
phase is `attached_unprompted` or `configuration_reverified`, terminal idempotency evidence
is absent, and no old value implies possible I/O. A stale/missing worker or
released lease with an identity-matched process is reaped then failed; ambiguous
process identity quarantines without signalling. Any terminal command-journal
or continuation result dominates queue status and prevents resurrection. A
terminal idempotency row with non-terminal queue/ledger state converges to that
terminal result and never re-enqueues.

The old engine writes `committed` and continuation `prompt_sent` before ACP I/O;
those values are therefore never positive evidence. Only the correlated
post-I/O receipt row above can produce migrated `prompt_sent`. A table-driven
generator enumerates the full finite cartesian product of every classifier axis
above for all three modes; invalid state-dependent tuples must reduce to typed
invalid/quarantine rather than being omitted. It asserts one classification,
turn/link nullability, process action, owner settlement, restart idempotency,
and replay exclusion for every generated row.

The legacy phase reducer is executable and exhaustive: each of `admitted`,
`launching`, `launched`, `attaching`, `attached_unprompted`, `prompting`,
`settling`, `cancelling`, `completed`, and `failed_closed` maps to the
byte-identical v2 phase. Dispatch truth is then reduced independently from the
ledger, receipt, process, release, and terminal axes above; phase alone never
proves send or zero-send. Checked-in goldens are produced independently by
`scripts/reference/p086_provider_truth_upgrade.py`, a stdlib-only implementation
that imports neither Rust code nor generated migration output. Mutation cases
remove each old phase and `released` state in turn and must fail coverage.

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

`ProviderTruthUpgradeCoordinator` is part of the DB-owned bootstrap session,
not a post-serve task. Production daemon startup calls exactly
`db::bootstrap::open_runtime_database(database_url)`. That lower-layer API
derives/acquires the database singleton lock and returns
`RuntimeDatabase { pool, preflight_lock_guard }`; the non-`Clone`,
non-serializable guard remains alive for the runtime's lifetime. Daemon
`supervisor` does not acquire a second database lock.

Inside that API, `run_preflight_with_guard(&mut PreflightLockGuard)` returns a
private `PreflightCompleteToken` only after migration, Rust finalization, and
reconciliation succeed. `create_pool_after_preflight(database_url, token)`
consumes that token and opens the runtime pool without calling preflight or
reacquiring the lock. The ordinary `create_pool` remains only for in-memory
tests and explicitly feature-gated maintenance binaries; a retained production
call-site scan rejects it in daemon startup. One-shot admin commands use the
same `open_runtime_database` path and keep the guard until exit.

The registered SQLx migration is deliberately a staging
migration: it creates `provider_truth_upgrade_state`, shadow/final-target tables
with nullable backfill columns, and compatibility read views, but installs no
constraint that requires Rust-derived data. Guarded preflight invokes the
coordinator after `Migrator::run` both when migrations were just applied and
when SQLx already reports the binary version as current. A tracked-equal DB with
phase other than `complete` must resume/fail the coordinator; the ordinary
equal-version return cannot bypass the Rust finalizer.

Before bounded work, guarded preflight proves its live lock token and opens
`BEGIN EXCLUSIVE`. It records target version, stable upgrade ID, and phase; copies every
source column used by a classifier into immutable
`provider_truth_upgrade_source_v1` tables; computes their schema-and-row digest;
and installs write-fence triggers on the original source tables that abort every
insert/update/delete while the marker is non-terminal. This transaction is the
real SQLite fence for daemon and acknowledged non-daemon writers. Batches read
only the immutable snapshot, recompute its digest before each checkpoint, write
only shadow/result tables, and advance a durable high-water mark. Restart
requires the same source digest, target version, upgrade ID, snapshot cardinality,
and trigger inventory; mismatch fails closed.

Once every row is classified, one final exclusive transaction rechecks the
snapshot digest and original-table fence, runs
`ProviderTruthSchemaFinalizer`, and rebuilds each
target table with final NOT NULL/CHECK/FK/unique constraints, copies only
validated rows, runs `foreign_key_check` plus invariant queries, switches the
compatibility views, removes fence triggers, and marks phase `complete`. No later
numbered SQL migration is needed to add those constraints. Only then does
preflight open the normal foreign-key-enforcing runtime pool. A failure in
staging, snapshot/fence creation, any batch, final copy, or verification leaves
the daemon in failed-serve with the immutable snapshot and durable marker;
consumers never observe an interim schema.

A real two-process fixture opens the same file-backed database. Process A
acquires the lower-layer guard, completes tracked-equal and subset finalization,
opens its pool through `create_pool_after_preflight`, and keeps serving without
self-reacquiring the flock. Process B cannot enter migration or open a writable
pool and receives the existing duplicate/anomalous-holder outcome. Killing A at
each preflight checkpoint releases the kernel lock; exactly one restarted
process resumes the durable marker. Trace assertions show one lock acquisition
for A's entire startup and zero nested attempts from pool creation.

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

Dynamic materialization classification is total rather than limited to pending
rows. The generated reducer crosses materialization status
`pending|running|terminal`, zero/one/multiple matching work items, old misnamed
column null/valid/dangling, true execution link null/valid/mismatched, source
provenance valid/missing/digest-mismatched, and execution/turn evidence. A
pending unique zero-send row receives migrated identity; a running row with
positive correlated send evidence links the execution/turn without replay; a
terminal row is readback-only; any duplicate, dangling, possible-delivery, or
contradictory row becomes `legacy_unverified` plus the narrow owner quarantine.
Every input cell produces exactly one classification and finalizer nullability
row, and crash fixtures resume before/after each dynamic batch checkpoint.

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

`CanonicalUtcTimestampV1` is exactly the ASCII grammar
`YYYY-MM-DDTHH:MM:SS.sssZ`: UTC `Z`, exactly three fractional digits, Gregorian
calendar validation, and no leap-second, offset, omitted fraction, or additional
precision. New receipt, acceptance, turn, topology-order, and report timestamps
in this proposal use it. Legacy timestamps are parsed strictly by the existing
decoder and projected once into this grammar without rewriting frozen snapshot
bytes; an unparseable value is legacy-unverified rather than lexically sorted.

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
  "verified_at": "2026-08-30T12:34:56.789Z"
}
```

Every key is required and contains a non-null string; both digest-shaped fields
are lowercase 64-character hex, `verified_at` is `CanonicalUtcTimestampV1`, and
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
`task_occurrence_id`, nullable `continuation_id`, `work_item_id`, nullable
`session_generation_id`, nullable
`provider_session_id`, `provider`, nullable `binding_fingerprint_sha256`,
`requested_model`, `requested_effort`, nullable `accepted_model`, nullable
`accepted_effort`, nullable `accepted_model_wire_value`, nullable
`accepted_effort_wire_value`, `configuration_state`, nullable
`acceptance_source`, nullable `source_generation_acceptance_sha256`, nullable
`verified_at`, nullable `failure_code`, and the non-negative integer
`prompt_dispatch_count_at_receipt`.

`configuration_attempt_index` and `prompt_dispatch_count_at_receipt` are
non-negative integers; the former must equal the owner's allocated attempt.

Receipt owner kind is closed to `agent_execution`, `p086_continuation`, or
`steward_agent_lane`;
receipt configuration state is closed to `configured`,
`failed_before_prompt`, or `cancelled_before_prompt`; acceptance source, when
present, is `fresh_negotiation`, `reused_session_generation`, or
`attached_session_reverification`. A configuring or
legacy projection has no receipt yet. These domains are JSON Schema enums, not
free strings.

For `configured`, both session IDs, binding fingerprint, all accepted/source
fields, source digest, and verification time are non-null, `failure_code` is
null, and the prompt count is zero. For `failed_before_prompt` or
`cancelled_before_prompt`, accepted/source/digest/time fields are null,
`failure_code` is non-null, the prompt count is zero, and session IDs plus the
binding fingerprint may be null only when settlement precedes their creation.
No unknown JSON keys are accepted. For owner kind `agent_execution`, both
execution/occurrence fields are non-null, continuation is null, and the tuple
matches the owning execution row. For `p086_continuation`, continuation,
execution, and occurrence fields are all non-null and match the continuation's
target tuple; owner ID equals continuation ID and the target execution's receipt
pointer is unchanged. For `steward_agent_lane`, execution, occurrence, and
continuation are null and the owner ID is exactly the durable lane ID whose
analysis, agent, provider, and work item match the invocation.
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
ports plus closed runtime outcomes and owns all live handles/process supervision:

- `PromptDispatchAuthorityPort` exposes owner/configuration settlement and the
  initial/final turn CAS; engine's `DurablePromptDispatchAuthority` implements
  it using `db` and is injected into the manager; and
- `ProviderRuntimeControlPort` exposes generation-scoped cancel/interrupt/reap
  results without DB types; `AcpRuntimeManager` implements it and engine injects
  that port into `DispatchInvalidationCoordinator`.

Thus `acp` never depends on engine/db, while engine's existing dependency on
`acp` is sufficient for both trait definitions and runtime control. Engine's
`ProviderInvocationCoordinator` is the sole service exposed to executors. It
calls the manager's authorized operation; the manager may call only the injected
authority port for permit/turn CAS and returns a closed
`ProviderRuntimeOutcomeV1` containing phase, byte certainty, generation/process
identity, cleanup requirement, and sanitized error. The coordinator then owns
DB owner settlement and calls the separate runtime-control port for bounded
cleanup. There is no manager-to-coordinator callback, re-entrant cleanup call,
or DB type in `acp`.

Daemon is the composition root: it creates the durable authority, exactly one
manager, the invocation/invalidation coordinator using the manager's
runtime-control port, and a bounded `FatalServeState` channel from authority to
daemon. The coordinator owns DB cancellation/epoch/owner transitions; the
manager owns tokens, handles, process groups, and transport. Neither port
exposes a raw session handle or permits ACP to mutate DB directly. If accepted
truth persistence and the minimal failure settlement both fail, authority sends
the fatal state before returning. The same rule covers every prompt-authority
double failure after I/O: if transport write/flush may have succeeded, final
`prompt_sent` CAS fails, and the separate `dispatch_unknown`/quarantine
settlement also fails, authority must publish
`FatalServeReason::PromptAuthorityUnsettledAfterIo` with the sanitized
owner/turn/process tuple. Returning an ordinary owner error is forbidden.
Daemon atomically flips readiness false, closes scheduler/continuation/Steward
consumers and all northbound listeners, rejects in-flight/new mutations, signals
all generation lifecycle tokens, and performs bounded identity-safe process
cleanup. It remains failed-serve until restart preflight persists unknown
settlement; no subsequent work item may mutate the database in that process.

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
| `original` | P017 mediation execution attempt ID | Mediation-owned execution/occurrence, mediation record, conflict, and InvokeAgent item all match and are running/active | Fail attempt/item; move mediation to `terminal_unverifiable(prompt_delivery_unknown)`; retain blocked conflict for operator settlement; never mutate a stage owner |
| `original` | P058 escalation execution attempt ID | Execution/occurrence, escalation ledger/tier attempt, frozen policy, active tier authority, and InvokeAgent item all match | Mark attempt/item failed; pause ledger with `provider_prompt_delivery_unknown`; terminalize tier retry authority; block stage/run without advancing tier |
| `code_writer_completion_repair` | P079 lease key | Parent execution/occurrence match; lease and InvokeAgent item are active | Mirror lease unknown; fail running item/execution; block stage/run |
| `output_contract_repair` | P079 lease key | Same P079 owner proof with generic repair event kind | Same P079 unknown settlement |
| `work_continuation_live_handle` | P086 continuation ID | Target execution/occurrence match even if execution is terminal; ProcessContinuation item running; continuation active and not cancelling | Mark continuation `needs_continuation_reconciliation`, fail item, preserve terminal parent execution, block stage/run |
| `work_continuation_resurrection` | P086 continuation ID | Same as live handle plus successful target-bound attach receipt | Same P086 settlement and close attached generation |
| `work_continuation_output_only` | P086 continuation ID | Frozen output-only mode, selected attachment proof, ProcessContinuation item running, source-edit prohibition frozen | Same P086 settlement; retain output-only evidence |
| `steward_analysis` | Steward lane ID | Matching StewardAnalysis item and lane are active; invocation carries the same analysis, lane, agent, provider, and work item; no prior turn exists | Mark only the lane `prompt_delivery_unknown`, then apply the sole Steward reducer: system unknown skips auditor and yields `Failed/Failed`; auditor unknown after valid system yields `Inconclusive/Completed`; forbid automatic replay |

P017 is a typed prompt owner, not generic stage settlement with a nullable stage
ID. One transaction reduces mediation, workflow conflict, mediation-owned
AgentExecution, InvokeAgent item, prompt turn, and the mediation retry-authority
row through this complete matrix:

| P017 outcome | Mediation / conflict | Execution / item / turn | Retry authority |
|---|---|---|---|
| Pre-prompt failure, bounded attempt remains | `queued` / `lead_mediation_pending` | failed / failed / `not_started` | consume one attempt and allocate a fresh execution, item, and turn |
| Pre-prompt failure, budget exhausted | `terminal_unverifiable` / `operator_confirmation_required` with reason `mediation_zero_send_exhausted` | failed / failed / `not_started` | terminalized |
| Sent, output valid and auto-settle permitted | existing P017 settled result / `resolved` | completed / completed / `prompt_sent` | terminalized-success |
| Sent, validation or confirmation required | `operator_confirmation_required` / `operator_confirmation_required` | completed-or-failed / completed / `prompt_sent` | terminalized; no automatic retry |
| Delivery unknown | `terminal_unverifiable(prompt_delivery_unknown)` / `operator_confirmation_required` with the same reason | failed / failed / `dispatch_unknown` | terminalized; no automatic retry |
| Cancellation | `canceled` / existing cancellation result | cancelled / cancelled / `not_started` or unknown by byte certainty | terminalized-cancelled |
| Supersession | `superseded` / `superseded` | cancelled / cancelled / `not_started` or unknown by byte certainty | superseded |

The paired `terminal_unverifiable` mediation and
`operator_confirmation_required` conflict is the sole representation of
"automatic mediation unavailable, operator action required"; neither row is
left `running`. No path blocks or completes an unrelated stage. Fixtures cover
every row and race cancellation/supersession against permit and final
settlement.

P058 uses the same authoritative turn but its own escalation reducer. The turn
stores the non-null escalation-ledger FK and owner ID is the tier's
AgentExecution ID. In one transaction the reducer updates the ledger, appends
an escalation event, settles the execution and InvokeAgent item, settles the
turn, and consumes or terminalizes the exact tier-attempt authority:

| P058 outcome | Ledger / event | Execution / item / turn | Next tier |
|---|---|---|---|
| Provable zero-send, bounded same-tier attempt remains | `active`; append `tier_zero_send_retry` | failed / failed / `not_started` | allocate fresh tier execution, item, turn, and attempt authority |
| Provable zero-send, no attempt remains | `paused(escalation_chain_exhausted)` | failed / failed / `not_started` | none; operator-only existing resume policy applies |
| Delivery unknown | `paused(provider_prompt_delivery_unknown)`; append unknown event | failed / failed / `dispatch_unknown` | none; automatic advance and deadline resume are forbidden |
| Sent terminal output | keep `active` until the existing P058 result classifier atomically selects next tier, completion, or exhaustion | terminal result / completed / `prompt_sent` | exactly one classifier transition |
| Cancellation or supersession | existing terminal cancellation/supersession event | cancelled / cancelled / byte-certain turn result | none |

The new pause reason is closed and operator-visible. It is not treated as
`escalation_deadline_elapsed`, so the existing explicit deadline-resume command
cannot reopen it. Crash fixtures cover every row before/after event append,
authority settlement, and next-tier enqueue and prove no tier advances twice.

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

`AcpRuntimeManager` owns one async `SessionPromptGate` and one
`GenerationLifecycleToken` per live generation, plus one non-shareable
`OwnerDispatchToken` for each admitted owner-kind, owner-ID, and prompt-turn-ID
tuple. `same_agent_family_within_run` may share a generation but
never an owner token. It also owns one absolute `ConfigurationDeadline` per
configuration attempt from allocation onward. The deadline is 30 seconds total
and is never reset. Spawn/barrier
release, Xcode broker/toolchain acquisition, `initialize`, `session/new`, every
configuration write/readback, authority persistence, and outcome handoff each
run under `tokio::select!` against that deadline, the generation token, and the
current owner token. Timeout/cancellation
records the exact last phase and returns a cleanup-required outcome; the engine
coordinator performs identity-safe cleanup and zero-send settlement under one
additional absolute 10-second cleanup deadline. An identity-ambiguous child is
quarantined rather than signalled. No broker request, authority call, settlement
await, transport task, or cleanup task may detach or outlive its owning
deadline. Configuration settlement uses a CAS over captured owner truth. Prompt
dispatch then holds the gate from permit through its separate fixed 10-second
transport write/flush deadline and final CAS; any final settlement/cleanup await
is bounded by the same absolute dispatch deadline or the explicit cleanup
deadline and cannot hang daemon shutdown.

Engine's `DispatchInvalidationCoordinator` is the only entry point for run cancellation,
stage/execution replacement, targeted retry cancellation, work-item
cancellation, direct session close, daemon shutdown, and resurrection cleanup.
It first commits the owner cancellation/supersession intent; run-wide
invalidation also increments `runs.prompt_dispatch_epoch`, while scoped
invalidation changes only the affected owner records. It then signals the
affected `OwnerDispatchToken`; only run-wide cancellation, daemon shutdown,
process-identity failure, or an explicit generation-wide settlement signals the
`GenerationLifecycleToken`. Transport write/flush runs under `tokio::select!`
with both applicable tokens and a fixed 10-second
write deadline and reports `zero`, `some`, or `unknown` bytes written. The
coordinator waits at most the same deadline for the gate, then asks the
supervised-process owner first for owner-scoped interruption. Runtime control
returns the closed result `owner_interrupted | generation_closed` with the exact
collateral owner/turn list. Only after durable settlement does it remove a live
handle.

If the provider cannot interrupt one request without closing the shared
generation, `generation_closed` is not silently treated as targeted success.
The coordinator atomically settles the cancelled owner by byte certainty and
handles each collateral owner: a collateral `not_started` turn remains
`not_started` and is rebound through a fresh checked generation with a new
receipt; a `prompt_sent` turn remains sent and its execution receives
`provider_generation_interrupted_by_scoped_cancel` for output recovery or its
existing retry policy; a collateral `dispatch_pending` turn is impossible
because `SessionPromptGate` permits only one active dispatch. No collateral
owner becomes `dispatch_unknown` merely because another owner was cancelled.
Raw `close_session`, `request_close_session`, `close_all_sessions`, adapter kill,
cancel, and supersede APIs remain private inside `acp`; the coordinator reaches
only typed generation-scoped runtime-control methods and consumes their bounded
results. Composition/call-graph tests reject a manager callback to engine, an
executor-to-manager bypass, an unbounded await in any named phase, and a daemon
that remains ready after a fatal persistence state.

If invalidation commits before the initial permit, epoch/owner CAS prevents all
bytes. If permit commits first, later invalidation may race with I/O and the
result is `dispatch_unknown` unless final `prompt_sent` committed first. This
ordering avoids deadlock and never claims zero bytes merely because cancellation
was requested.

The retained A/B race matrix uses two occurrences sharing one
`same_agent_family_within_run` generation. It crosses A before/after permit with
B `not_started` or `prompt_sent`, and runtime control
`owner_interrupted|generation_closed`. A settles cancelled/unknown according to
its own bytes; B either keeps sent truth or rebinds from not-started truth, never
inherits A's failure, and never becomes unrelated `dispatch_unknown`. Run-wide
and fatal generation cancellation still settle every owner through the
generation token.

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

Fault injection covers a provider whose flush completes, final CAS fails, and
unknown/quarantine settlement fails independently. It proves the fatal channel
is delivered before the invocation future returns, readiness becomes unhealthy,
HTTP/MCP/GraphQL listeners and all consumers close, no later mutation commits,
the identity-matched child is reaped, and restart writes one unknown settlement
before reopening. The same fixture runs for original, P017, P058, both P079
kinds, all P086 modes, and Steward.

Crash/restart behavior is frozen at each durable boundary:

| Last durable boundary | Startup action | Prompt replay |
|---|---|---|
| Owner/turn reserved; no launch intent | Settle typed preparation failure; an original owner may use only its frozen workflow retry budget, while Steward uses `max_zero_send_retries = 1` and its durable consumed counter | Only through a new checked claim with a fresh turn |
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
  INVOKE_AGENT P017_MEDIATION P058_ESCALATION P079_REPAIR P086_CONTINUATION
  STEWARD_AGENT_LANE
}
enum ProviderConfigurationAcceptanceSource {
  FRESH_NEGOTIATION REUSED_SESSION_GENERATION ATTACHED_SESSION_REVERIFICATION
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
type ProviderExecutionTruth {
  schemaVersion: String!
  agentExecutionId: ID
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  executionProvider: String
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
extend type QueryRoot {
  providerExecutionTruthSchemaVersion: Int!
}
extend type GqlAgentExecution {
  providerExecutionTruth: ProviderExecutionTruth!
}
extend type RunStageTopologyOccurrence {
  presentationRowId: ID!
  compiledTaskId: ID!
  taskOccurrenceId: ID
  occurrenceSequence: Int
  occurrencePosition: TopologyOccurrencePosition!
  activeExecutionId: ID
  providerExecutionTruth: ProviderExecutionTruth!
  executionAssociationState: TopologyExecutionAssociationState!
  legacyAmbiguousExecutionCount: Int!
}
enum TopologyExecutionAssociationState {
  MATCHED_V2 LEGACY_UNIQUE LEGACY_AMBIGUOUS NOT_STARTED
}
enum TopologyOccurrencePosition {
  PLANNED CURRENT PREVIOUS
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
  providerExecutionTruth: ProviderExecutionTruth!
}
```

`ProviderExecutionTruth` is the only new execution-truth object on GraphQL.
Agent execution, mediation attempt, and topology occurrence all resolve that
same Rust object and field set; no surface re-declares or renames its members.
The generated alias map is exact: each lower-camel GraphQL member above maps to
the same-name snake-case JSON member, and each GraphQL enum maps to the closed
lowercase wire value. IDs, turn fields, summary containers, lists, and booleans
shown with `!` are non-null; historical rows may retain null execution/
configuration scalars. Topology still derives non-null presentation/compiled
IDs from frozen or migration identity even when no execution exists.

Aggregation reduces every authoritative turn and every runtime-receipt row, not
just the latest turn. `ProviderPromptDeliveryReducerV1` is complete and ordered:

1. any `dispatch_unknown` turn yields `UNKNOWN`;
2. otherwise any `legacy_unverified` receipt yields `LEGACY_UNVERIFIED`;
3. otherwise select the greatest `(turn_index, prompt_turn_id)` specialized
   repair/continuation turn. `not_started|dispatch_pending` yields its
   `REPAIR_PENDING|CONTINUATION_PENDING` value and `prompt_sent` yields its sent
   value;
4. only when no specialized turn exists, an original `dispatch_pending` or
   `prompt_sent` yields `ORIGINAL_PENDING` or `ORIGINAL_SENT`; and
5. all authoritative turns `not_started`, or a planned occurrence with no
   execution, yields `NOT_STARTED`. A historical execution with no turn is
   `LEGACY_UNVERIFIED`, not zero-send.

Therefore an original sent turn never hides a later repair/continuation pending
turn. Original truth remains independently available in `originalTurnState`.
Unknown and unverified evidence dominate every positive or pending state.
Generated fixtures enumerate every original/specialized state combination,
multiple repair/continuation indices, and shuffled row order.

`noPromptSent = true` only when every authoritative turn is `not_started` and
there is no unknown or unverified receipt. A runtime receipt may degrade
confidence or confirm a linked turn, but receipt count, null receipt, or a
provider-specific prompt counter can never positively prove zero-send. This
rule is identical for Codex, Claude, Gemini, Auggie, and Junie. A planned
topology occurrence with no execution is `NOT_STARTED`, true, false; an
empty-turn historical execution is `LEGACY_UNVERIFIED`, false, false.

Receipt linkage is never collapsed to an unexplained scalar. Each linked turn
exposes its own nullable `runtimeReceiptLinkState`; unlinked historical receipts
remain outside the turn array. `runtimeReceiptLinkSummary` counts all execution
receipts, including those unlinked rows, and computes `worstState` by the total
order `LEGACY_UNVERIFIED > LEGACY_PRE_PROMPT > LINKED_V2`; it is null only when
the execution has no runtime receipts. Counts are non-negative and sum to the
receipt row count. Thus original, both repair kinds, and every continuation can
have different link truth without losing evidence. Reducer fixtures enumerate
empty, homogeneous, and every mixed link-state multiset, including multiple
unlinked rows and sent-turn conflicts.

Execution association is explicit. A v2 occurrence match is `MATCHED_V2`; the
single-task historical `agent_id` fallback is `LEGACY_UNIQUE`; more than one
candidate is `LEGACY_AMBIGUOUS`, leaves `activeExecutionId` and all runtime
identity fields null, and reports the bounded candidate count; no candidate is
`NOT_STARTED`. The latest execution within a matched occurrence is selected by
the total order `started_at DESC, id DESC` using `CanonicalUtcTimestampV1` bytes
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
`task_occurrence_sequence`, nullable
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

The internal canonical name remains `reports.get`; Codex-compatible
`tools/list` may expose its existing `reports_get` alias, which must canonicalize
back to the same capability. Its `McpTool` carries a non-null Draft 2020-12
`outputSchema` for exact `ReportsGetResultV1`: an object with only required
`schema_version = "reports_get_result_v1"`, required ordered `reports` array,
and required `p082_recovery_matrix_readbacks`. The last field preserves the
existing top-level response lane and must equal the same-named member in the
`mcp_execution_truth` report; it is not moved or dropped. The report array is a
closed versioned union that preserves every pre-change report variant and
field. Its `mcp_execution_truth` member preserves the exact existing keys
`id`, `run_id`, `stage_id`, `agent_id`, `name`, `contract_id`, `format`,
`artifact_metadata_pointer`, `checksum_sha256`, `size_bytes`, `provider`,
`model`, `created_at`, `is_pinned`, `report_kind`, `report_version`,
`agent_executions`, `code_writer_completion_receipts`,
`implementationCompletion`, `workflow_conflict`, `retryAuthority`,
`retryAuthorityHistory`, `p091OrphanRepairReadback`,
`implementation_handoff_status`, `implementation_self_assessment_summary`,
`rollout_contract_readback`, `p082_recovery_matrix_readbacks`,
`p080_reconciliation`, `implementation_closeout_readiness_summary`,
`closeout_readiness_summary`, and `temp_artifact_inventory`. The only additive
change inside it is `provider_execution_truth` on each execution. The existing
`canonical_artifact_contracts` report variant likewise retains its current
keys. The `mcp_execution_truth` execution field embeds the shared
`provider_execution_truth_v1` schema by `$ref`; `additionalProperties` is false
for the versioned top-level and nested truth objects, while preserved legacy
report variants use their checked-in exact schemas. `McpTool` serializes
`inputSchema` and `outputSchema` with
those exact MCP spellings. `tools/list` copies both fields rather than rebuilding
a three-field object that drops output schema.

`tools/call` returns typed `CallToolResultV1`, not the current text-only wrapper:
`content` is one text item containing canonical JSON of `ReportsGetResultV1`,
`structuredContent` is that same object, and optional `isError` is absent on
success. The generic dispatch path requires `structuredContent` and validates it
against the registered output schema for every tool that declares one; schema or
text/object mismatch fails before JSON-RPC success. Fixtures execute real
`initialize`, `tools/list`, and `tools/call` for both canonical and alias names,
assert advertised `outputSchema`, and byte-compare decoded text,
`structuredContent`, report resource, generated artifact, and GraphQL-mapped
domain DTO.

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
author execution truth or the canonical file. Frozen compilation exposes a
distinct provider output `run_report_candidate` at the exact per-execution path
`${CHAINWORKS_META_ROOT}/candidates/<lead_agent_execution_id>/run-report.json`.
The provider prompt and `ExecutionRequest` contain only that candidate path;
logical artifact `run_report` and its canonical path are absent from provider
outputs, environment, and tool arguments.

Every production provider subprocess runs under the runtime-owned filesystem
profile that denies writes to the complete `${CHAINWORKS_META_ROOT}` tree and
then grants only its exact candidate file plus any source-worktree permissions
required by the frozen agent profile. A pre-opened candidate dirfd is bound to
the execution and path traversal/symlink replacement fails closed. The
canonical report directory has no provider write grant. A call-site and prompt
snapshot gate rejects any adapter or output directive that exposes the
canonical path.

`engine::RunReportMaterializer` is the sole final writer of logical artifact
`run_report`. This migration creates
`artifact_materialization_leases` keyed by `(run_id, logical_artifact_name)` with
lease generation, holder daemon generation, candidate hash, state
`active|committed|released`, expiry, and timestamps; a partial unique index
allows one active writer. After candidate validation and after all
AgentExecution settlement, the materializer acquires that engine-owned
`run_report` lease by CAS,
removes any agent-supplied `provider_execution_truths`, inserts the authoritative
ordered array from the domain serializer, and canonicalizes the complete JSON.
Before replacement it persists a materialization journal with candidate hash,
truth-array hash, intended final hash, target path, and state `prepared`. It
writes a same-directory temporary file, fsyncs it, atomically renames, fsyncs the
directory, then transactionally updates artifact metadata/checksum, journal
state `committed`, and lease state `committed`. Startup may steal only an expired
active lease by generation-CAS after proving its holder daemon is not current;
it verifies a prepared row against target/temp hashes and either completes the
exact rename/metadata commit or restores the validated candidate, then releases
the lease terminally. The ordinary artifact writer cannot acquire the same
logical key while materialization is active, and the lead cannot overwrite a
committed report. Readback rehashes canonical bytes against committed metadata;
an out-of-band mismatch is `run_report_integrity_failed`, quarantines the
artifact, and never presents changed bytes as accepted truth. Tests race two
materializers, kill at every lease/journal/write/fsync/rename/metadata boundary,
and run a fake provider that writes its candidate, attempts the undisclosed
canonical path before and after commit, and modifies the candidate late. Both
canonical writes are denied by the OS profile, the late candidate mutation is
ignored after the prepared candidate hash, and one final canonical checksum
and byte-equal embedded DTO array survive. A separate trusted tamper fixture
proves checksum mismatch blocks readback rather than updating metadata.

`run://` no longer serializes database records directly. The checked-in
`docs/reference/schemas/run-resource-readback-v1.schema.json` freezes two exact
principal projections after run-scope authorization:

| Principal | Exact top-level keys |
|---|---|
| Operator | `schema_version`, `id`, `idea_id`, `status`, `workflow_id`, `workflow_title`, `workspace_root`, `artifact_root`, `started_at`, `completed_at`, `cancellation_requested_at`, `cancellation_settled_at`, `cancellation_settlement_log`, `current_state`, `workflow_yaml_path`, `agent_catalog_yaml_path`, `worktree_root`, `base_branch`, `base_revision`, `target_branch`, `delivery_configuration_json`, `delivery_preflight_json`, `workflow_family`, `project_key`, `risk_class`, `stack`, `workflow_snapshot_hash`, `catalog_snapshot_hash`, `workflow_snapshot_json`, `catalog_snapshot_json`, `drift_detected_at`, `drift_details_json`, `chainworks_meta_root`, `review_routing_json`, `closeout_readiness_mode`, `active_artifact_index`, `run_state_projection`, `operator_overrides`, `rollout_contract_readback`, `total_stages`, `completed_stages`, `failed_stages`, `pending_approvals`, `implementation_self_assessment_summary`, `stages`, `escalation_readback` |
| Agent or Observer | `schema_version`, `id`, `idea_id`, `status`, `workflow_id`, `workflow_title`, `started_at`, `completed_at`, `cancellation_requested_at`, `cancellation_settled_at`, `current_state`, `workflow_family`, `project_key`, `risk_class`, `stack`, `drift_detected_at`, `closeout_readiness_mode`, `total_stages`, `completed_stages`, `failed_stages`, `pending_approvals`, `stages`, `escalation_readback` |

`schema_version` is always `run_resource_readback_v1`. Nullable current fields
are emitted as explicit null and projection counters default to zero, so key
presence does not depend on database history. Agent and Observer key sets are
byte-identical after authorization; their `escalation_readback` is the existing
summary-only form. Unknown keys fail schema validation.

Nested stage objects are also exact. Operator stages preserve current keys
`id`, `run_id`, `stage_id`, `label`, `status`, `iteration`, `attempt_number`,
`settlement_kind`, `started_at`, `completed_at`, `owner_agent`, `provider`,
`model`, `stage_type`, `validation_failure_json`, `evidence_packet_json`,
`recovery_snapshot_json`, `retry_reason`, and `agent_executions`. Agent/Observer
stages omit the four diagnostic/evidence keys
`validation_failure_json|evidence_packet_json|recovery_snapshot_json|retry_reason`
and preserve every other key.

`RunAgentExecutionReadbackV1` for Agent/Observer has exactly `id`,
`stage_execution_id`, `task_occurrence_id`, `task_occurrence_sequence`,
`agent_id`, `provider`, `model`,
`started_at`, `completed_at`, `status`, and `provider_execution_truth`. The
Operator variant preserves those plus all current execution diagnostics:
`owner_execution_lineage_id`, `session_lineage_id`, `session_generation_id`,
`rehydrated_from_checkpoint_artifact_id`, `invocation_owner_key`,
`session_reuse_scope`, `session_family_id`, `session_reuse_disposition`,
`session_reset_reason`, `backend_profile_id`, `requested_mcp_extensions_json`,
`predicted_mcp_extensions_json`, `predicted_mcp_runtime_ids_json`,
`actual_mcp_extensions_json`, `actual_mcp_runtime_ids_json`,
`denied_mcp_extensions_json`, `mcp_blocking_issues_json`,
`actual_mcp_observation_json`, `actual_xcode_runtime_observation_json`,
`mcp_session_startup_latency_ms`, `owner_kind`, `owner_id`,
`lead_mediation_record_id`, `origin_stage_execution_id`, `total_cost_cents`,
`input_tokens`, `output_tokens`, `cached_input_tokens`,
`transcript_artifact_id`, `actual_toolchain_mapping_diagnostics_json`,
`escalation_policy_id`, `escalation_policy_hash`, `escalation_tier_id`,
`escalation_tier_kind_raw`, `escalation_trigger_raw`,
`escalation_digest_version`, and `escalation_ledger_id`.

`report://` and `reports.get` remain Operator-only under the existing report
authorization boundary. GraphQL uses the same safe execution DTO for
Agent/Observer callers and adds operator diagnostics only through separately
authorized fields. Fixtures assert the exact key arrays for all three principal
classes and cross-run scope denial before serialization; non-Operators cannot
receive provider-session IDs, raw receipts, completion/workflow/retry/P091/
rollout diagnostics, stage evidence, transcript/cost/toolchain data, or artifact
payload lanes.

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
| acceptance source | `fresh_negotiation`, `reused_session_generation`, `attached_session_reverification` |
| prompt kind | `original`, `code_writer_completion_repair`, `output_contract_repair`, `work_continuation_live_handle`, `work_continuation_resurrection`, `work_continuation_output_only`, `steward_analysis` |
| prompt owner kind | `invoke_agent`, `p017_mediation`, `p058_escalation`, `p079_repair`, `p086_continuation`, `steward_agent_lane` |
| prompt dispatch state | `not_started`, `dispatch_pending`, `prompt_sent`, `dispatch_unknown` |
| delivery truth | `not_started`, `original_pending`, `original_sent`, `repair_pending`, `repair_sent`, `continuation_pending`, `continuation_sent`, `unknown`, `legacy_unverified` |
| runtime-receipt link | `linked_v2`, `legacy_pre_prompt`, `legacy_unverified` |
| topology association | `matched_v2`, `legacy_unique`, `legacy_ambiguous`, `not_started` |
| topology occurrence source | `static_compiled`, `owner_compiled`, `dynamic_materialized`, `legacy_flat` |
| topology occurrence position | `planned`, `current`, `previous` |

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
  array, including an empty legacy array; topology with no execution has a
  non-null `ProviderExecutionTruth` shell whose execution/requested/accepted
  scalars are null plus a non-null `NOT_STARTED` summary, with configuration
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
| Pending | `Planned: Claude - opus - High` |
| Before or during dispatch | `Starting: Claude - opus - High - Acceptance unverified` |
| Startup failure before dispatch | `Start failed: Claude - opus - High - No prompt sent - Acceptance unverified` |
| Cancelled before prompt | `Cancelled before prompt: Claude - opus - High - No prompt sent - Acceptance unverified` |
| Prompt sent / running | `Running: Claude - opus - High - Acceptance unverified` |
| Prompt sent / completed | `Completed: Claude - opus - High - Acceptance unverified` |
| Prompt sent / failed | `Failed: Claude - opus - High - Acceptance unverified` |
| Prompt sent / cancelled | `Cancelled: Claude - opus - High - Acceptance unverified` |
| Dispatch unknown | `Prompt delivery unknown: Claude - opus - High - Do not retry automatically` |
| Repair pending/sent | Status prefix plus `Repair starting` or `Repair prompt sent` and `Acceptance unverified` |
| Repair unknown | `Repair prompt delivery unknown: Claude - opus - High - Do not retry automatically` |
| Continuation pending/sent | Status prefix plus `Continuation starting` or `Continuation prompt sent` and `Acceptance unverified` |
| Continuation unknown | `Continuation prompt delivery unknown: Claude - opus - High - Do not retry automatically` |
| Historical execution | Status prefix plus requested identity and `Delivery unverified` |

A Codex-to-non-Codex fallback uses the provider-neutral row and never inherits
the prior Codex accepted pair. A non-Codex-to-Codex fallback must complete the
exact Codex transaction. Missing model or effort segments are omitted, not
invented.

Cancellation while still `not_started` is provably unprompted. Cancellation
after `dispatch_pending` but before durable `prompt_sent` settles
`dispatch_unknown`; cancellation must not erase delivery ambiguity.

### Display mapping

Provider full-display canonicalization is total: raw `codex`, `claude`, `gemini`,
`auggie`, and `junie` display as `Codex`, `Claude`, `Gemini`, `Auggie`, and
`Junie`; an unknown non-empty provider is preserved byte-for-byte in full
output. Only the
three explicit Codex model mappings below change model spelling. Every other
provider model, including raw `opus`, is preserved exactly; the formatter never
title-cases, trims, aliases, or locale-folds an unknown/raw model.

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
- Run Inspector summary, execution-attempt rows, and identity detail popovers;
- Timeline agent headers whenever execution identity is shown; and
- Help and accessibility labels derived from those rows.

Run Inspector deletes its separate truncated planned-model formatter. It uses
the same accepted/requested state input and `fullIdentity`/`compactIdentity`
result as Overview and Stages, including effort, verification state, prompt
truth, and the complete unknown raw value in Help/copy/accessibility output.

The formatter returns both `fullIdentity` and `compactIdentity`.
`fullIdentity` always preserves unknown raw provider/model/effort values and is
the sole source for Help, accessibility, copy, and the detail popover.
`compactIdentity` is independently bounded for every unbounded segment. Each
unknown non-empty segment always becomes exactly
`Custom provider sha256:<prefix>`, `Custom model sha256:<prefix>`, or
`Custom effort sha256:<prefix>`; it never embeds raw unknown bytes. Known provider names,
Sol/Terra/Luna, generic GPT-5.6, and known effort labels are never abbreviated.

Each prefix is the first 10 lowercase hex characters of SHA-256 over
`UTF8(domain) || 0x00 || raw UTF-8 bytes`, with domains
`chainworks.custom_provider_label.v1`,
`chainworks.custom_model_label.v1`, and
`chainworks.custom_effort_label.v1`. Empty/missing segments are omitted and are
not hashed. This bounds each unknown compact segment to 33, 30, or 31 ASCII
characters respectively and the complete compact identity to its fixed status
prefix plus three such segments. Fixtures include all three domains, ASCII,
non-ASCII, leading/trailing whitespace, empty values, cross-domain equal raw
bytes, and values sharing the
first eight digest characters to enforce the ten-character rule.

The checked-in formatter input corpus is generated from the closed legal-state
table, not hand-picked examples. Expected output is computed independently by
`scripts/reference/provider_execution_identity_formatter.py`, a stdlib-only
oracle that parses the committed input JSON and implements the closed state
matrix, mappings, domain hashes, nullability rejection, and string assembly
without importing Swift/Rust production code, generated production output, or
the checked-in expected file. It includes every configuration state, execution
status (`pending`, `running`, `completed`, `failed`, `cancelled`), delivery
truth, provider class (exact Codex, legacy Codex, non-Codex), every known model
and effort, unknown non-empty values, and missing optional non-Codex segments.
For every legal tuple it freezes status prefix, `fullIdentity`,
`compactIdentity`, Help, copy, and accessibility strings; every illegal tuple
must return typed `identity_contract_invalid` rather than a best-effort label.
The gate proves each enum value appears in at least one legal golden and one
applicable mutation-negative. Visual, Help, copy, and accessibility outputs are
then byte-compared to the same formatter result.

The gate runs the oracle into a temporary file, runs the Swift production
formatter over the same inputs, and byte-compares both canonical JSON outputs
to the committed golden. Mutations change each status branch, known mapping,
hash domain, NUL separator, prefix length, nullable key, and segment order; the
oracle and production outputs must diverge or validation must fail. Regenerating
expected output by calling the production formatter is forbidden by a retained
import/call-site scan.

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

The tie-break vocabulary is exact. `NodeKeyV2` is
`(frozen_workflow_ordinal, stage_id UTF-8 bytes)`. Tarjan visits roots by
`NodeKeyV2` and outgoing edges by target node key, transition ordinal, then
transition ID; members of each completed SCC are sorted by node key.
`SccKeyV2` is the minimum member `NodeKeyV2` (SCCs are disjoint, so it is
unique). A weak component key is its minimum SCC key. Condensation edges sort by
`(source SccKeyV2, target SccKeyV2, transitionOrdinal, transitionId)`.

Sweep positions are zero-based integers from the immediately preceding order.
Every median is stored as `median2`, twice its exact value: an odd neighbor count
uses `2 * middle_position`, an even count uses
`lower_middle_position + upper_middle_position`, and no neighbors use
`2 * prior_position`. Medians compare as integers; equal
medians retain prior order, then `NodeKeyV2`. After the fourth sweep, nearest
free track minimizes `abs(2 * track - median2)` over non-negative free
integer tracks; equal distance chooses the smaller track. SCC members occupy
consecutive tracks in member-key order and reserve the complete block before
the next SCC is placed.

A long edge crossing boundary ordinal `b` gets virtual ID
`topology_virtual_v2:<sha256>` over common-codec components
`[transition_id, canonical_base10(b)]`; `b = 0` is the first boundary after the
source column. Virtual keys sort by boundary column, transition ordinal,
transition ID, then `b`. A self-loop ordinal is its zero-based index among self-loop
transition IDs on the same node sorted by `(transitionOrdinal, transitionId)`.
Outer cycle channels sort by `(SccKeyV2, transitionOrdinal, transitionId)`.
These keys also resolve symmetric fork/merge layouts; geometry or input order
never breaks a tie.

No dictionary iteration, input array order, measured card height, or special
workflow ID participates in placement. Permuting identical graph input must
produce byte-equal placement.

Geometry constants are frozen in `StageTopologyMetricsV2`: column width 320 pt,
minimum column gap 88 pt, minimum track height 96 pt, track gap 16 pt, ordinary branch
channel spacing 12 pt, outer-cycle inset 24 pt, and self-loop top inset 16 pt.
Coordinates snap to half-point boundaries. Every edge uses `midTrailing` as its
source port and `midLeading` as its target port. For each adjacent column
boundary, normalize all crossing forward segments by transition/virtual-node
order and let `n` be their count. Its exact gap is
`max(88, 24 + 12 * (n - 1))` for `n > 0`, otherwise 88; later column x-origins
are cumulative widths and boundary gaps. Channel `i` is
`leftMaxX + (gap - 12 * (n - 1)) / 2 + 12 * i`, `0 <= i < n`, then half-point
snapped. Therefore ordinal 4 and every later channel remain inside the widened
gap with at least 12 pt edge clearance. Back/cycle edges use the right outer SCC channel
`graphMaxX + 24 + 12 * channel_ordinal`, ordered by SCC key then transition ID.
A self-loop with ordinal `i` has exact orthogonal points
`[midTrailing, (maxX+24+12i, sourceY), (maxX+24+12i, minY-16-12i), (minX-24-12i, minY-16-12i), (minX-24-12i, targetY), midLeading]`.
Long-edge virtual nodes use the same per-boundary inventory and dynamic-gap
rule. Parallel/self-loop fixtures freeze every point, port, channel index, gap
width, cumulative column origin, and half-point rounding.

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
orthogonal branch junctions use their assigned per-boundary channel x. The same
global frames drive manually paired branches, hit testing, focus, popovers, and
accessibility. No connector computes y-position from a fixed card-height
constant.

Pure layout tests cover a fork, diamond merge, two-node cycle, self-loop,
long-edge virtual nodes, disconnected components, equal even medians, equidistant
free tracks, and shuffled input. Symmetric fork/merge and mirrored-cycle vectors
permute nodes and edges exhaustively and freeze SCC keys, virtual IDs, self-loop
ordinals, `median2` values, chosen tracks, and outer channels. Hosted
tests use the real full-MVP graph with mixed 1/2/5-occurrence cards and assert
non-overlap, stable tracks, actual-frame connector centers, bounded crossings,
and no fallback to the removed hard-coded map.

`scripts/reference/stage_topology_layout_v2.py` is the independent layout
oracle. It implements normalization, SCC/rank/track assignment, dynamic gaps,
channel coordinates, and half-point rounding from the frozen input JSON without
importing Swift outputs. The Swift pure-layout suite and Python oracle each emit
canonical result JSON and must byte-match checked-in goldens. Mutations alter an
ordinal, array order, source ID, channel count, gap constant, card height, and
rounding boundary; each must change the expected field or fail validation rather
than regenerate a matching golden from production output.

If the v2 topology projection is absent, schema-invalid, or carries an unknown
source/order value, the hosted view renders the exact state
`Topology unavailable` with action `Retry after daemon restart`; it renders neither the
old hard-coded map nor sequential guessed nodes. A hosted negative fixture
asserts this state, no connector/card accessibility elements, and successful
recovery when a valid projection replaces it.

The layout golden corpus crosses each graph shape with occurrence counts
`0, 1, 2, 5, 32, 256`, forward-channel counts `1, 4, 5, 8, 64`,
one-line/two-line/unknown-long identity rows, and transition
ordinals that do not match SQL input order. Every case freezes node/edge IDs,
columns, tracks, spans, edge channels, measured frames, connector endpoints,
focus order, and presentation-row IDs. A seeded stress/property corpus reaches
1,024 occurrences and 256 transitions per boundary. Required invariants are:
byte-equal output under 100 deterministic input shuffles; no frame/channel
overlap; every edge
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
`topology_row_v2:<sha256>` over exact components
`[run_plan_identity_marker, state_id, source_kind, source_stable_id, compiled_task_id, row_scope, row_scope_id]`. A source with no occurrence uses
`row_scope = "planned"` and `row_scope_id = source_stable_id`. Each durable
occurrence uses `row_scope = "occurrence"` and
`row_scope_id = task_occurrence_id`; retry/fallback attempts within that
occurrence keep the same row ID, while loop/replacement occurrences receive
distinct IDs.
For new rows the marker is SHA-256 of duplicate-key-rejected RFC 8785 `RunPlan`
JSON with runtime IDs/timestamps absent, not the workflow file hash; legacy uses
the exact persisted marker above. All four source kinds use the unified
projection. A planned row is removed when its first occurrence is created; that
occurrence row then has its own stable identity and never changes as attempts
arrive. Every Overview, Stages, Run Inspector, active-agent, and Timeline DTO
carries the same `presentationRowId`, nullable `taskOccurrenceId`, nullable
`occurrenceSequence`, and `planned|current|previous` position. Selection,
popover state, and Timeline filtering key by presentation row ID, never by agent
ID. Two same-agent occurrences therefore remain distinct rows with independent
status, attempts, model, and effort. When a selected planned row is replaced,
focus moves to the `current` occurrence of the same source; otherwise removal
uses the deterministic next-row rule above. Visual, Help, popover, copy, and
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
| Final sent CAS and unknown/quarantine settlement both fail after possible I/O | `PromptAuthorityUnsettledAfterIo`; daemon failed-serve | unknown |
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
after `dispatch_pending`. Unknown delivery applies the exact owner reducer:
ordinary/P017/P058/P079/P086 work items fail and their run-bound scopes block as
specified above; Steward first settles its lane and then derives the shared
analysis/work-item result from system-versus-auditor position. It forbids
automatic replay without inventing a run.
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
| `workflow` + `domain` | Exact seven-profile matrix; per-profile frozen capabilities and effective fallback contract; validated Steward catalog; fresh generic/invalid rejection; legacy replay; sealed provenance union; exact nine-ID manifest; typed P058 owner; `OutputContractRepair` work-item kind; complete compiled-coordinate, condition, binding, dynamic-key, occurrence/sequence, and presentation vectors |
| `acp` fake provider + `engine` dispatch | Response-closed negotiation; single-flight execution/P086/Steward configuration ownership; acyclic typed outcome/control ports and permit-only API; bounded broker/config/send/settlement/cleanup; complete P017/P058/P079/P086/Steward reducers; owner-scoped versus generation cancellation; launch barrier/process identity; bidirectional fallback; Claude aliases unchanged |
| `db` + `engine` recovery | Lower-layer lock-guarded staged/tracked-equal preflight, immutable source snapshot/write-fence/finalizer and restart marker; active owner attempts/receipts/failures; exact Steward retry/reducer; prompt authority/quarantine; P079 operation/attempt/link migration; exhaustive old P086 phase/released classifier with independent oracle; sealed legacy envelopes; total dynamic rebuild; closure-owned replay authorization |
| `daemon` composition | One `open_runtime_database` guard and no lock reacquisition; real production construction of upgrade coordinator, durable authority, ACP manager, invocation/invalidation coordinators, process-control port, and fatal-state channel; no missing/default authority or direct handle path; consumers/listeners close on finalization or every double persistence failure |
| `graphql-server` + `mcp-server` | Byte-equal complete `AppSchema::sdl()` plus probe matrix; one non-null nested GraphQL truth shell; complete latest-specialized-turn reducer; exact versioned `ReportsGetResultV1` preserving P082; principal-specific `run://` key arrays; exact `outputSchema`, `tools/list`, typed `tools/call`, resources, structured-content parity, and redaction |
| Swift focused and hosted-view tests | Presence-aware DTO decoding; lockstep restart; complete state/ambiguity/start-failure matrices; one formatter plus independent stdlib oracle across Overview/Stages/Run Inspector/Timeline; distinct same-agent occurrence row keys and selection; exact SCC/median/track/virtual/self-loop rules; topology-unavailable and stress states; exact keyboard/focus/copy/accessibility contracts |

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
    prompt only through one fresh negotiation; P086 attachment instead performs
    generation-scoped option reverification and never copies acceptance;
11. original success followed by each of
    `code_writer_completion_repair` and `output_contract_repair` proves
    independent durable turns, typed `OutputContractRepair` lifecycle,
    work-item-bound lease-v2 parity, one logical budget across bounded zero-send
    attempt leases, attempt-scoped parent links, TTL behavior, atomic fallback
    child execution/turn creation, and attempt-0 migration of the old globally
    unique links;
12. P086 admission atomically creates command journal, continuation, work item,
    turn, and reserved side effect; failure at each insert rolls back all and
    idempotent replay returns the same IDs;
13. live-handle, resurrection, and output-only P086 modes reserve distinct
    typed turns, require target execution/occurrence and ProcessContinuation
    item, own an independent configuration allocator/generation/receipt pointer,
    preserve frozen mode across attachment conversion, mirror side-effect rows
    only after CAS, reject fresh-session fallback, fail before I/O on wrong
    identity, and independently reproduce every old phase plus
    `planned|started|committed|released|failed` migration outcome;
14. a Steward claim persists the running analysis and both lane owners before
    either provider call; `system_steward` and `steward_auditor` then use
    owner-scoped lineage/process/configuration/turn records without synthetic
    run/execution IDs, while a historical unowned running item is terminalized
    and quarantined without replay; every two-lane/cancellation combination
    reduces to existing `Completed|Inconclusive|Failed|Superseded` status, and
    crash/replay proves the durable one-retry zero-send cap rejects attempt two;
15. every initial/final combination of `Applied`, `AlreadyMatching`, `Conflict`,
    and `Missing` enforces owner/generation/request binding, including initial
    commit-ack loss, final commit-ack loss, zero-byte conflict, and missing-row
    quarantine;
16. a transport that never completes write/flush is cancelled within the fixed
    deadline, broker/toolchain/authority/settlement/cleanup awaits are bounded,
    the invalidator never waits indefinitely on the gate, out-of-band process
    cleanup runs, and no public raw close/kill/cancel API bypass exists;
17. production API compilation proves `AcpSession`, handles, raw prompt/start/
    execute methods, and direct adapter fallback are unavailable outside `acp`;
    the sole manager send site consumes a one-use authority permit;
18. Claude, Gemini, Auggie, and Junie advance the shared prompt ledger while
    keeping provider-configuration truth non-applicable;
19. cancellation races use two owners on one `same_agent_family_within_run`
    generation and prove owner-only cancellation or deterministic collateral
    rebind preserves B's not-started/sent truth without unrelated unknown; both
    directions of cross-provider fallback preserve occurrence association;
20. the launch barrier plus process-binding crash table is exercised at every
    boundary for original, both repairs, all continuations, and both Steward
    lanes; only identity-matched processes are reaped and no prompt is replayed;
21. cancellation during configuration, run/targeted-retry invalidation
    immediately before permit, and invalidation racing after permit are
    linearized by the generation gate; run-wide races additionally prove epoch
    fencing, while scoped races prove unrelated stages retain their epoch;
22. a real two-process file-DB startup proves one lower-layer lock acquisition,
    no `create_pool` preflight/lock reacquisition, exclusive finalization, and
    deterministic takeover after crash; startup keeps consumers closed and applies every receipt/work-item/Steward/
    P079/P086/dynamic row in the persisted-work matrix, runs on subset and
    tracked-equal schema states, rechecks immutable snapshot/fence digests, reconciles processes and
    unknown turns, and only then enables claims;
23. the generated replay manifest covers every enqueue/claim/requeue/prompt
    site and all selectors refuse unresolved, quarantined, stale, absent, or
    migration-pending authority;
24. all nine exact producer IDs, same-owner retry/fallback, replacement-stage
    retry, loop re-entry, dynamic idempotency/conflict, `legacy_flat`, and
    migration-only `LegacyInvokeEnvelopeV1` behavior match the complete
    component arrays and independently reproduced golden vectors; P058 tier
    changes compile distinct binding identity while ordinary runtime fallback
    never alters compiled identity;
25. canonical JSON tests include the RFC 8785 known answer, number/string
    vector, duplicate-key rejection, digest mismatch, pre-session failure,
    malformed receipt, exact millisecond UTC grammar, all three link states,
    v1/v2 decode, and unsupported receipt version;
26. a generated legal/nullability matrix and mutation-negatives prove every
    closed enum and receipt/DTO combination;
27. fresh `AppSchema::sdl()` byte-matches the full checked-in snapshot with the
    actual Rust type names; schema probe action/error handling is executable;
28. real `tools/list` advertises `reports.get` output schema and real
    `tools/call` returns schema-valid equal text/structured content that
    preserves top-level `p082_recovery_matrix_readbacks` and byte-matches
    `run://`, `report://`, mediation, and generated run-report JSON; exact
    Operator/Agent/Observer run-resource key arrays and the nested non-null
    GraphQL shell map back to the same domain value;
29. transition ordinal/ID and association-state fixtures prove shuffled-query
    parity, the complete latest-specialized-turn reducer (including original sent
    plus repair/continuation pending), authoritative-turn-only zero-send, planned
    `NOT_STARTED`, legacy unique, and legacy ambiguous behavior; and
30. pure and hosted layout/formatter corpora cover Overview, Stages, Run
    Inspector, Timeline, distinct same-agent loop/replacement occurrence rows,
    deterministic current/previous selection, all legal states, fork,
    merge, cycle, self-loop, long edge, disconnected graph, 100 input shuffles,
    mixed heights, minimum width, 200% text, keyboard popover focus restoration,
    and row removal.
31. staged migration interruption at every batch/finalizer boundary and the
    tracked-equal post-staging restart resumes from the durable marker; external
    SQLite writes hit the fence and snapshot/digest drift fails closed;
32. each owner kind races configuration admission and proves one active
    attempt/generation/process, one winning receipt/permit, monotonic gaps, stale
    pointer rejection, and `receipt_unavailable` cleanup/startup recovery;
33. every P079 lease kind/state/budget/FK/classifier combination and every P086
    mode/status/phase/worker/process/heartbeat/release/ledger/attachment/receipt/
    terminal-idempotency combination reaches one frozen result without treating
    a pre-I/O write as sent;
34. system/auditor lanes cover valid report, missing report, no executor,
    configuration failure, unknown delivery, cancellation, and crash replay;
    both lanes and the analysis work item always become terminal;
35. Codex-to-non-Codex and non-Codex-to-Codex fallback persist target-derived
    effective contracts while preserving compiled/occurrence identity;
36. tool/resource JSON-RPC envelopes, advertised `outputSchema`, typed
    `structuredContent`, nested GraphQL alias map, every mixed receipt-link
    multiset, and Operator/Agent/Observer redaction round-trip through the DTO;
37. run-report materialization races and kill points converge through one
    run/artifact lease to one authoritative file, while a fake provider receives
    only a candidate path, canonical writes before/after commit are denied, late
    candidate writes are ignored, and trusted tamper blocks readback;
38. daemon production composition proves the actual authority/coordinator/process
    ports/fatal channel are installed before listeners, rejects cyclic callback
    wiring, fails construction when absent, and leaves serve mode when final sent
    CAS and subsequent unknown settlement both fail after transport flush;
39. all four topology source kinds, pre-execution legacy rows, frozen/unverified
    exact source IDs, legacy marker, Overview order, ordinals, ambiguous
    association, post-configuration start failure, and
    topology-unavailable recovery execute in pure and hosted tests; and
40. condition canonical bytes, timestamp grammar, RunPlan/presentation hashes,
    independently bounded provider/model/effort labels, exact SCC keys,
    even-median/nearest-track tie breaks, virtual IDs, self-loop ordinals,
    dynamic connector gaps/channels/ports, symmetric permutation and stress
    cases, and accessibility labels match the named independent Python and Swift
    golden implementations and fail mutation checks.

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
- [ ] AgentExecution, P086 continuation, and Steward owners each reserve exactly
      one active configuration attempt/generation/process tuple and bind the
      winning receipt/permit to their own pointer.
      Receipt persistence failure has a separate zero-send failure row,
      deterministic process cleanup, and startup convergence without fabricating
      or transferring authority.
- [ ] Matching live-session generation evidence is projected before reuse;
      missing or mismatched evidence closes the old session with zero prompts
      on that handle and permits only one fully negotiated fresh fallback for
      an eligible original owner, never P079 or P086. Attached P086 generations
      reverify both options and persist their own acceptance before a permit.
- [ ] Session lineage, provider session, and process binding support typed
      run-agent, P086-continuation, and Steward-lane owners. Launch intent/barrier and PID/start
      identity are durable before `session/new`, and crash recovery never
      signals an identity-ambiguous process.
- [ ] Production crates cannot construct a prompt permit or access raw
      `AcpSession`/handle/start/prompt/execute fallbacks; the sole manager send
      site consumes one authority-backed permit. Runtime outcomes and cleanup
      ports form an acyclic call graph, every await is bounded, and both
      pre-prompt and post-flush final-CAS/unknown-settlement double persistence
      failures drive daemon failed-serve through the fatal channel.
- [ ] `provider_prompt_turns`, not terminal runtime receipts or P079/P086 domain
      rows, is the sole dispatch authority. Ordinary, typed P017, and typed P058
      original prompts, both repair kinds, all three continuation modes, and
      both Steward prompts receive independent durable turns.
- [ ] Historical runtime receipts remain representable as `linked_v2`,
      `legacy_pre_prompt`, or `legacy_unverified`; only positive unique evidence
      creates a sent turn and new writes always carry a valid turn FK.
- [ ] P079 operation/lease v2 changes atomically with its turn, consumes one
      logical budget across bounded zero-send attempt leases, uses the typed
      `OutputContractRepair` work item, keys links by operation/attempt, creates
      fallback child execution/turn before lease binding, exhaustively reduces
      eligibility, and migrates old links to attempt 0 without dropping history.
- [ ] P086 admission atomically commits command, continuation, work item, turn,
      and reserved side effect. All three modes carry the real execution and
      occurrence, mode survives attachment conversion, and no side effect can
      claim sent before the final CAS. Rebuilt continuation and ledger tables
      accept every old/new phase and classify worker/process, heartbeat,
      `released`, terminal idempotency, and old pre-I/O evidence with an
      independent oracle and without inventing send truth.
- [ ] Dispatch and cancellation share the invalidation coordinator and one
      generation gate plus owner-scoped dispatch tokens; run-wide invalidation
      additionally uses the durable epoch/lifecycle token. Targeted cancellation
      cannot mark another owner on a shared generation unknown and rebinds a
      collateral not-started owner deterministically.
- [ ] Transport write/flush is cancellation-aware and bounded to ten seconds;
      invalidation can interrupt the supervised process out of band, cannot
      deadlock on the gate, and is the only public close/kill/cancel path.
- [ ] The complete initial/final four-result CAS table covers commit-ack loss,
      zero-byte conflict, missing-row quarantine, and final unknown settlement.
- [ ] The complete launch/configure/permit/write/final/receipt crash table
      converges to one process owner and deterministic settlement for every
      prompt kind without duplicate provider I/O.
- [ ] Unknown delivery marks the owning item failed, blocks a run-bound owner or
      records typed P017/P058/Steward settlement, and is excluded by every automatic
      retry, continuation, fallback, claim, and startup-requeue selector.
- [ ] Startup keeps every consumer closed until the complete legacy row matrix,
      dynamic rebuild, receipt states, Steward/P079/P086 migration, process
      reconciliation, and unknown quarantine finish; no old running item can be
      replayed first.
- [ ] SQLx staging, tracked-equal restart, immutable source snapshot, SQLite
      write-fence triggers, digest checks, Rust backfill, final rebuild, durable
      marker, and failed-serve behavior form one executable preflight protocol;
      a DB-owned lock guard and consumed preflight token prevent self-reacquisition
      and a second process; replay authorization cannot escape its owning closure
      or transaction.
- [ ] A generated `ReplaySelectorIdV1` manifest covers every prompt-capable
      enqueue/claim/requeue/send site; new unclassified production paths fail
      the structural gate.
- [ ] Every one of the nine production `InvokeAgent` source classes delegates
      to typed enqueue/claim validation; `legacy_flat` is explicit, same-owner
      retry/fallback preserves identity, a new stage execution recomputes it,
      and every P058 tier with a changed binding receives a distinct coordinate.
- [ ] `LegacyInvokeEnvelopeV1` is migration-only and absent from
      `ProducerIdV1`; compiled coordinates exclude owner, materialization,
      provider fallback, and loop-instance data, and complete golden arrays are
      independently reproducible.
- [ ] Dynamic materialization persists compiled-task, occurrence, and work-item
      identity atomically, migrates the historically misnamed column without
      treating a work-item ID as an execution ID, and fails closed on conflict.
- [ ] Identity codecs and receipt schemas freeze exact fields and bytes; golden
      SHA-256 vectors, duplicate-key rejection, RFC 8785 known answers, verified
      digests, exact millisecond UTC grammar, named independent encoders,
      mutation checks, and the v1/v2 decoder matrix are executable fixtures.
- [ ] The app proves schema v1 before the new GraphQL document, performs at most
      one bundled-daemon replacement/retry, and fails visibly on persistent
      mismatch.
- [ ] GraphQL and Swift distinguish planned, configuring, configured but not
      started, prompt sent, delivery unknown, failure, cancellation, and legacy
      states without treating non-Codex null configuration as an error.
- [ ] GraphQL exposes one nested truth object on execution, mediation, and
      topology. MCP `tools/list` advertises the exact reports output schema and
      typed `tools/call` returns equal text/structured content while preserving
      top-level P082 readback. Canonical parity, latest-specialized-turn
      reduction, exact principal key arrays, non-null topology shells, complete
      SDL, and Swift omitted-versus-null behavior match checked-in fixtures.
- [ ] The engine, not the lead agent, materializes authoritative execution truth
      from a provider-only candidate into `run_report` under a
      run/logical-artifact lease and crash-consistent journal/rename/checksum
      flow; provider sandboxes cannot write the canonical path and late candidate
      writes cannot change accepted bytes;
      daemon-owned composition installs the real authority and process-control
      ports before listeners or consumers open.
- [ ] Codex/non-Codex fallback in either direction never inherits incompatible
      accepted truth and remains tied to the original occurrence.
- [ ] Full model/effort identity is identical across visual, Help, popover,
      copy, and accessibility output in Overview, Stages, active-agent rows, Run
      Inspector, and Timeline; provider/raw-model spelling is total, compact text
      bounds unknown provider/model/effort independently, never abbreviates a
      known Sol/Terra/Luna pair, matches an independent stdlib oracle, and focus
      survives removal.
- [ ] Deterministic graph placement replaces the hard-coded/sequential maps and
      proves all four source kinds, exact pre-execution IDs and legacy marker,
      normalized Overview order, frozen ordinals,
      distinct same-agent occurrence rows/current-previous selection, exact
      condition/presentation bytes, SCC/median/track/virtual/self-loop tie-breaks,
      fork, merge, cycle, self-loop channels,
      dynamically widened high-cardinality channels, long-edge, stress,
      disconnected, shuffled, legacy ambiguity, start-failure and
      topology-unavailable states, exact accessibility labels, and real
      mixed-height topology with actual-frame connector centers.
- [ ] `./scripts/test-gate.sh codex-model-truth` passes with nonzero Rust and
      independently nonzero required pure and hosted Swift test execution.
