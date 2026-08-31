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
- Change MCP protocol negotiation, `run://`, `report://`, `reports.get`, tool
  output schemas, generated run-report bytes, artifact materialization, or
  provider filesystem/sandbox policy. Those surfaces retain their current wire
  and authorization behavior and continue to expose only their existing
  planned/requested compatibility fields.
- Prove real keyboard or VoiceOver event delivery through remote XCUITest. This
  slice proves formatter/accessibility values and the shared selection/focus
  reducer in pure and hosted-view tests; OS-level interaction remains under the
  existing remote UI gate, not this proposal's release signoff.
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
| `codex_ops_low` | `gpt-5.6-luna` | `high` | Bounded operational work with the approved reasoning floor |

No other backend profile changes in this slice.

The current Codex ACP effort vocabulary is `low`, `medium`, `high`, `xhigh`,
`max`, and `ultra`. All six values remain recognized and tested. The approved
profile matrix intentionally starts at `high`: no current Chainworks role is
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
- `production.p079_fallback` carries producer ID
  `p079.provider_fallback_child`, source work-item ID/envelope digest,
  source compiled-task and occurrence IDs, repair operation ID, attempt index,
  lease key, parent failed execution ID, frozen fallback-policy hash, and
  selected fallback binding digest; and
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

The exact production `ProducerIdV1` vocabulary is frozen to the ten IDs in the
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
| `p079.provider_fallback_child` | preserve | Exact validated source `compiled_task_id` and `task_occurrence_id`; require the typed P079 fallback provenance and selected target binding digest |

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

Ordinary provider fallback is a binding change on one of those producer-owned
invocations, not another producer. The P079 fallback child is the one explicit
tenth producer route because it creates a separately lease-owned child with
typed `production.p079_fallback` provenance. Both cases preserve the source
compiled-task and occurrence IDs. A targeted retry with a new stage execution
and every loop re-entry preserve the compiled-task ID but recompute occurrence
from the new owner. The enum-generated manifest must byte-match the checked-in
ten-ID inventory, so adding an eleventh variant fails the gate until its
identity and behavior fixture are added.

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
new ID, so attempts from different owner scopes cannot be merged. The enqueue
transaction, before the work-queue row becomes visible, uses
`TopologyOccurrenceAllocator` to assign non-negative `occurrence_sequence`,
monotonically increasing per `(run_id, source_stable_id)`. It inserts immutable
`invoke_agent_occurrences(run_id, source_stable_id, task_occurrence_id,
occurrence_sequence, compiled_task_id, envelope_digest)`, writes the same ID and
sequence into the validated envelope/work item, advances the allocator, and
commits all four together. It does not create an `AgentExecution`.

Claim reparses the envelope and copy-validates every occurrence field against
`invoke_agent_occurrences` before changing queue status. Only then does the
claim transaction create the first `AgentExecution` and copy the immutable ID
and sequence into its row/topology projection; retries and P079 fallback
children copy the same occurrence while creating new execution attempts. An
idempotent enqueue returns the existing sequence and never allocates a second
value for the same task-occurrence ID. Crash after enqueue but before claim
leaves one queued occurrence with no execution; restart claims it without
allocating again. Crash before enqueue commit leaves none. Pending static/owner
topology exposes `compiled_task_id`; its `task_occurrence_id` and sequence remain
`null` until the enqueue transaction creates the occurrence.

Legacy sequence backfill is deterministic and restartable. Under the preflight
lock and SQLite write fence, `ProviderTruthUpgradeCoordinator` first writes an
immutable `provider_truth_occurrence_sequence_stage` snapshot. It deduplicates
all derivable historical rows by
`(run_id, source_stable_id, task_occurrence_id)` and stores
`LegacyOccurrenceOrderKeyV1` with these exact ordered components:
`stage_iteration_or_minus_one`, `stage_attempt_or_minus_one`,
`canonical_owner_created_at_key`, `owner_kind_rank`,
`owner_id_lowercase_uuid`, `canonical_source_work_item_created_at_key`,
`source_work_item_id_lowercase_uuid`, and `task_occurrence_id`.
Owner-kind rank is `stage = 0`, `mediation = 1`,
`escalation = 2`, `dynamic = 3`, and migration-only work-item owner `= 4`.
Each timestamp key is `0:` when the nullable source timestamp is absent or
`1:<CanonicalUtcTimestampV1>` when present. Present timestamps must parse and
re-encode through `CanonicalUtcTimestampV1`; malformed bytes make that owner
legacy-unverified rather than changing its order by locale. The same tagged
encoding applies to `canonical_source_work_item_created_at_key`.

Within each `(run_id, source_stable_id)` partition, zero-based
`occurrence_sequence` is `row_number - 1` in that total order. The stage table
stores the source row count, ordered canonical digest, assigned sequence, and
batch state. Bounded backfill upserts envelope, work-item, AgentExecution, and
topology projection by occurrence ID and compares any existing value; it never
allocates during an unordered source query. Only after every staged row matches
does one finalizer create/update `topology_occurrence_allocators` with primary
key `(run_id, source_stable_id)` and `next_sequence = max(sequence) + 1` (or
zero for an empty partition). New runtime allocation is a Class A owner-creation
CAS against that row. Crash before/after any batch or allocator seed reuses the
same staging digest and produces byte-identical sequences. Shuffled source
queries, duplicate attempts within one occurrence, loop/replacement owners, and
`max + 1` allocation are retained fixtures. A pending row whose source or order
key cannot be derived follows `invoke_agent_upgrade_identity_missing`; a
terminal ambiguous row keeps nullable sequence and never enters a v2 envelope.

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
`frozen_task_ordinal` on every occurrence. It also assigns a stage-scoped,
non-null `human_source_ordinal` to every source row. Static and owner-compiled
sources receive consecutive zero-based values in normalized compiler order.
Each stage persists `next_human_source_ordinal` immediately after that range;
dynamic materialization atomically consumes one value with the immutable source
row, so two dynamic tasks can never share a spoken ordinal even when their
template `frozen_task_ordinal` is the same. Retry/fallback/loop occurrences keep
their source's human ordinal. Legacy migration sorts distinct source rows by the
tagged total order below, assigns one consecutive value per source, records
`legacy_order_unverified` where applicable, and seeds the allocator at verified
`max + 1`. The ordinal is presentation truth only and never enters compiled-task
or occurrence identity.

State ordinal is source YAML state
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

The gate combines the structural scan with behavior tests for all ten exact
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
8. Drain every already-received `session/update` notification through the same
   session actor, require its `config_option_update` payload to parse, and
   reverify the exact pair against the resulting live option snapshot.
9. Atomically persist generation-scoped
   `ProviderConfigurationAcceptanceV1` and owner-scoped
   `ProviderConfigurationReceiptV1`; for a run agent, project the same receipt
   onto its `AgentExecution` in that transaction. The acceptance captures the
   live option snapshot revision and digest.
10. Permit `session/prompt` only after receipt persistence succeeds and the
    session actor still holds the same valid snapshot revision.

An empty/malformed option response, missing option, unknown value, incompatible
effort after model selection, send failure, provider rejection, current-value
mismatch, or persistence failure is a typed startup failure with zero prompt
dispatch. A successfully returned JSON-RPC response without matching
`currentValue` is not acceptance.

`config_option_update` is authority-bearing input, not logging. The per-session
transport actor owns `GenerationOptionSnapshotV1`: generation ID, monotonic
local revision, exact ordered model and effort options/current values, last
provider update sequence, validity, and canonical digest. The actor alone
applies `session/new`, set-option responses, and
`session/update.config_option_update` notifications in wire-observation order.
Unknown fields are retained only in a bounded diagnostic digest; a missing,
duplicate, malformed, or contradictory model/effort option invalidates the
snapshot and generation. The actor calls the registered
`provider_configuration.invalidate` authority operation before another permit
can be issued; the operation appends invalidation evidence and updates the
generation plus every still-active owner receipt projection by exact generation
and snapshot revision. A pre-prompt persistence failure is zero-send; a
post-write invalidation persistence double-failure closes the synchronous fatal
admission fence.

The one-use prompt permit captures snapshot revision and digest. Immediately
before writing prompt bytes, the same actor compares the permit to its current
snapshot under the generation prompt gate. A changed or malformed update before
write revokes acceptance and returns `ACP_PROVIDER_CONFIGURATION_INVALIDATED`
with zero prompt bytes. An update observed after write starts but before the
terminal provider response settles the turn as delivery/configuration unknown,
invalidates generation reuse, and blocks automatic replay; accepted fields
remain historical negotiation evidence but are not rendered as actual runtime
identity. An update after terminal settlement invalidates the generation for
the next owner without rewriting the completed turn. Fake transports inject an
update before model response, between both set-option responses, after receipt
commit/before prompt, during write, during response, and after terminal
settlement, including malformed and reordered forms.

### Live-session reuse

A new prompt on an existing Codex session does not repeat `session/new`, so
the accepted pair is owned by the durable `SessionGeneration`, not only by the
first `AgentExecution`. The generation stores:

- provider-configuration contract version;
- accepted model and effort;
- provider-session ID and binding fingerprint;
- accepted-at timestamp;
- bounded `ProviderConfigurationAcceptanceV1` and its SHA-256 digest.

Before a reused prompt, the engine submits
`provider_configuration.reserve_existing_generation`. Its atomic transaction
loads the active generation and requires all of these values to match the live
handle and current request: generation ID, provider-session ID, provider,
binding fingerprint, contract version, requested model, requested effort, and a
still-valid `GenerationOptionSnapshotV1` revision/digest. It also requires the
configuration owner's active attempt/generation fields to be null, reads
`next_configuration_attempt_index = n`, and inserts exactly one logical owner
binding for the already-created generation and prompt turn with attempt `n`.
It does not allocate a generation or advance the physical session-generation
allocator.

When they match, that same transaction derives a new owner-bound
`ProviderConfigurationReceiptV1` from the generation acceptance and writes it
to the authoritative receipt table with `configuration_attempt_index = n`,
moves the owner's current-receipt pointer, advances
`next_configuration_attempt_index` to `n + 1`, and leaves the active pair null.
For a run agent it also writes the exact
receipt projection to the new `AgentExecution` with
`acceptance_source = reused_session_generation` and the source acceptance
digest. The derived receipt names the new agent execution and task occurrence;
it does not copy the first execution's IDs. That projection is
response-verified authority inherited from the same live provider session; it
is not a new negotiation.

`provider_generation_owner_bindings.prompt_turn_id` is globally unique, not
merely unique within a generation. Its active-owner key is also unique on
`(prompt_owner_kind, prompt_owner_id, prompt_turn_id)`. A prompt turn can
therefore reserve either one new generation or one existing generation, never
both. The binding stores configuration owner kind/ID, allocated attempt index,
and receipt ID. Idempotent replay with the same generation returns that existing
binding, receipt, and attempt index without advancing the allocator; the same
turn with another generation is `Conflict`. Races between reuse,
invalidation, cancellation, and new-generation fallback are resolved in that
single reservation transaction before any process/session/prompt I/O.

When evidence is absent, stale, malformed, or mismatched, the manager closes
and invalidates the generation before any prompt. Only an original InvokeAgent
whose frozen ordinary-owner policy explicitly allows compatibility recovery may
then perform at most one fresh-session fallback through the complete negotiation
transaction. P079 and P086 fail closed because they require the parent or
attached generation. Steward also has no transparent fallback: the current turn
fails zero-send and only the lane's explicit one-retry authority may allocate
turn `1`, after which the common configuration allocator creates a fresh
generation. The old session receives zero prompts.

P086 provider-session resurrection never copies acceptance from the source
daemon generation into a newly attached generation. The only supported attach
seam in this slice is the installed `@agentclientprotocol/codex-acp` 1.1.7 ACP
contract. Before the source generation can become resumable, the adapter
persists secret-safe `ProviderSessionResumeContextV1` with schema version,
context ID, source generation ID, an internal provider-session-row reference,
provider/adapter contract version, target binding fingerprint, canonical
absolute `cwd`, ordered canonical `additionalDirectories`, and an immutable MCP
descriptor-set reference plus RFC 8785 digest. The descriptor set contains
names/transports and references to broker-owned secret inputs, never raw tokens,
expanded environment secrets, or a northbound provider-session ID. The source
generation stores the context ID/digest; P086 admission copies both into the
continuation in the same transaction as its frozen target binding. A
missing/mismatched context or digest rejects admission before process I/O.

The supervised child is then launched and bound to target process identity;
only its correlated `initialize` response can prove
`agentCapabilities.sessionCapabilities.resume`. Missing capability or an
initialize failure is `ACP_P086_RESUME_UNSUPPORTED`, followed by bounded
identity-safe reap and zero prompt bytes. There is no pre-launch capability
claim. After capability proof, the adapter sends exactly one `session/resume`
request populated only from the admitted immutable context: the internally
resolved stored provider session ID, exact canonical `cwd`, complete ordered
`additionalDirectories`, and complete frozen MCP descriptor set after
broker-side secret resolution. `session/load`, `session/new`, omitted roots/MCP,
admission-time recomputation from the current workspace, or an adapter-private
attach method is forbidden on this path.

The correlated `session/resume` response must arrive for that request and must
contain a non-empty, completely parseable `configOptions` array. Although ACP
makes this member optional generally, P086 exact-pair attachment makes it
required. The response seeds `GenerationOptionSnapshotV1` at local revision
zero. An authority-bearing `config_option_update` observed after the resume
request but before its response is causally ambiguous: it is not buffered,
reordered, or applied after the response, and instead fails closed with
identity-safe reap and zero prompt bytes. Only updates whose transport sequence
is strictly after the correlated response may be applied in observation order;
every accepted update increments the local revision. A missing/duplicate model
or effort option, skipped invalid item, malformed notification, response/session
correlation mismatch, pre-response update, duplicate/regressed sequence, or
option update that cannot be ordered is
`ACP_P086_RESUME_CONFIGURATION_UNAVAILABLE` and closes the attached generation
with zero prompt bytes.

Only after this catalog source is established does the manager reserve the
continuation owner's single active configuration attempt and run the normal
model-first set/readback sequence against the resumed session, without
`session/new`. Response-verified equality may create the new generation
acceptance and owner receipt with
`acceptance_source = attached_session_reverification`. The attach receipt,
active attempt, new generation, process binding, acceptance, option snapshot,
and continuation turn form one authority tuple before a permit. If the provider
cannot re-read and confirm both options, attachment fails zero-send; old
generation evidence is never transferred by ID or digest. Fake ACP fixtures
cover missing/mismatched admitted context, launch/initialize/capability failure,
resume error, omitted/empty/partially invalid options, session mismatch,
pre-response update rejection, ordered post-response update, invalidation
during both set calls, and the accepted exact pair. Every post-launch negative
asserts identity-safe reap; every negative asserts no `session/prompt`,
`session/new`, or `session/load` write and no source acceptance transfer.

Legacy v0 generations may be reused only by a
`legacy_best_effort_v0` execution. A `codex_exact_pair_v1` request never
inherits legacy-unverified generation evidence.

## Durable Runtime Truth

Migration 100 adds these columns to `agent_executions`:

| Column | Meaning |
|---|---|
| `task_occurrence_id` | Stable occurrence shared only within one owner scope |
| `task_occurrence_sequence` | Monotonic source-scoped presentation sequence allocated with the occurrence |
| `requested_model` / `requested_effort` | Canonical pair requested for this execution |
| `accepted_model` / `accepted_effort` | Canonical response-verified pair; otherwise `null` |
| `accepted_model_wire_value` / `accepted_effort_wire_value` | Exact provider option values whose `currentValue` was verified |
| `provider_configuration_state` | `configuring`, `configured`, `invalidated_after_acceptance`, `failed_before_prompt`, `cancelled_before_prompt`, or `legacy_unverified`; `null` for non-Codex |
| `provider_configuration_verified_at` | Complete-pair verification time; otherwise `null` |
| `provider_configuration_invalidated_at` / `provider_configuration_invalidating_snapshot_sha256` | Durable option-update invalidation evidence; otherwise `null` |
| `provider_configuration_receipt_json` / `provider_configuration_receipt_sha256` | Bounded projection of the authoritative owner-scoped receipt and its verified digest |
| `acceptance_source` | `fresh_negotiation`, `reused_session_generation`, or `attached_session_reverification`; otherwise `null` |
| `configuration_evidence_state` | Non-null `pending`, `receipt_available`, `invalidated`, `receipt_unavailable`, `not_applicable`, or `legacy_unverified` |
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
  verified-at fields plus live option snapshot revision/digest/validity and
  invalidation timestamp;
- creates durable `steward_agent_lanes` before any Steward provider call and
  rebuilds `provider_sessions` plus new `provider_process_bindings` so both run
  and non-run generations have typed process ownership;
- creates `provider_configuration_receipts`, the owner-scoped accepted-pair
  authority described below; `agent_executions` stores only its lockstep
  projection;
- creates `provider_configuration_failures` for every terminal zero-send
  configuration failure or cancellation before acceptance, including an
  authoritative receipt that could not be committed;
- creates append-only `provider_configuration_invalidations` keyed by
  generation and observed option-snapshot revision, with prior/current digest,
  observation phase, malformed/change reason, byte-certainty, and timestamp;
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
`zero_send_retry_pending`, `prompt_sent`, `completed`,
`configuration_failed`, `prerequisite_skipped`, `cancelled_before_prompt`,
`failed`, `prompt_delivery_unknown`, and `legacy_unverified`.
`zero_send_retry_pending` is non-terminal and is reachable only from the typed
retry reducer below; the last seven values are terminal.

At claim, `run_steward_analysis_with_executor` computes all deterministic
analysis inputs and, in one transaction, inserts the `steward_analyses` row as
`running`, inserts both lane rows as `reserved`, and binds the already claimed
StewardAnalysis work item. That transaction also allocates the system lane's
turn `0`; the auditor lane remains turnless until a validated system health
report permits its own Class A initial claim, which allocates auditor turn `0`.
Provider calls happen only after the corresponding turn commit. Final
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
| cancellation/replacement wins before a turn exists or while the active turn is `not_started`, including `zero_send_retry_pending` before a replacement turn exists | settle each non-terminal lane as `cancelled_before_prompt`; retain prior zero-send turns | `Superseded` / `Cancelled` |
| cancellation/replacement wins while the active turn is `dispatch_pending` | settle that lane as `prompt_delivery_unknown`; skip/reduce its counterpart by the prerequisite rules | `Superseded` / `Cancelled` |
| cancellation/replacement wins after the active turn is `prompt_sent` | preserve `prompt_sent`, settle the lane `failed(cancelled_after_prompt)`, and never rewrite delivery to unknown | `Superseded` / `Cancelled` |

Any pair not matching one row is `steward_lane_reduction_invalid`, leaves the
work item failed, and fails startup/readback verification. No terminal or
skipped lane is eligible for startup requeue. Cancellation, system failure,
auditor failure, missing output, and crash after either lane settlement each
have an executable row fixture.

The only automatic Steward replay is a zero-send infrastructure retry. Initial
claim allocates turn index `0`; its permit requires
`zero_send_retries_consumed = 0`, exactly that active turn, and no earlier turn.
An eligible typed infrastructure failure leaves turn `0` permanently
`not_started` with its non-null failure code, appends matching
`provider_configuration_failures` cleanup evidence, proves the process reaped
or never launched, and moves the lane to `zero_send_retry_pending`. It does not
return a terminal lane directly to `reserved`.

One Class A `steward_lane.claim_or_retry` transaction may then CAS
`zero_send_retries_consumed: 0 -> 1`, allocate only turn index `1`, and move the
lane to `reserved`. After that commit, the common
`provider_configuration.reserve_new_generation` operation separately allocates
the new configuration attempt, generation, and process-binding intent for the
exact lane/turn tuple. The retry permit requires exactly one earlier turn, turn `0`; that
turn must remain `not_started`, carry the closed zero-send failure code, have no
receipt or unknown side effect, and join the reaped/never-launched generation.
It also requires the new turn `1`, consumed counter `1`, and no cancellation or
supersession. The old turn is evidence and is never reused or deleted. A second
retry, a skipped/repeated turn index, delivery unknown, positive/ambiguous I/O,
or failure to prove cleanup settles through the table above and cannot requeue.
Crash fixtures stop before and after failure settlement, retry counter CAS,
new-turn insert, launch barrier, cancellation in each dispatch state, and lane
terminal settlement and prove one retry and at most one prompt write.

Fresh-generation policy is closed: an ordinary initial owner may use only its
frozen ordinary recovery allowance; Steward turn `0` gets one generation and
the proven zero-send retry turn `1` gets one different generation; P079 repair
must use the parent generation; P079 fallback child gets only its atomically
admitted initial generation; and P086 must use its live or attached generation.
No lower transport/configuration helper may allocate another generation for any
of these owners.

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

The singular owner on a lineage/session is its physical lifecycle custodian,
not exclusive prompt authority. Session reuse adds
`provider_generation_owner_bindings`, keyed by
`(session_generation_id, prompt_owner_kind, prompt_owner_id, prompt_turn_id)`.
It stores the logical owner tuple, work-item ID, occurrence when run-bound,
configuration-owner kind/ID, non-negative configuration-attempt index,
nullable `configuration_receipt_id`, nullable `configuration_failure_id`,
nullable terminal reason, binding state `admitted`, `configured`,
`waiting_for_prompt_gate`, `dispatching`, `awaiting_terminal`, `terminal`, or
`cancelled`, and timestamps. A generation may therefore have many sequential
logical owners while each owner/turn belongs to exactly one generation. The
final rebuild installs both references only after their target tables exist and
enforces this exhaustive matrix:

| Binding state/result | Receipt ref | Failure ref | Additional rule |
|---|---:|---:|---|
| `admitted` | null | null | Prompt turn is `not_started`; no configuration terminal evidence exists |
| `configured`, `waiting_for_prompt_gate`, `dispatching`, `awaiting_terminal` | non-null | null | Receipt owner/attempt/generation exactly matches the binding |
| `terminal` after configured/prompt path | non-null | null | Terminal reason is one of the closed post-configuration results |
| `terminal` after configuration failure | null | non-null | Failure owner/attempt/generation exactly matches; prompt turn remains `not_started` |
| `cancelled` before configuration acceptance | null | non-null | Failure code is `cancelled_before_configuration`; prompt turn remains `not_started` |
| `cancelled` after configuration acceptance but before prompt | non-null | null | Receipt remains historical acceptance; prompt turn remains `not_started` |

Exactly one reference is therefore required after a binding leaves `admitted`,
and both references are always forbidden. Insert/update triggers reject a
receipt/failure owner, attempt, generation, work-item, or prompt-turn mismatch;
reject a dispatch-capable state without a receipt; reject a pre-configuration
terminal failure whose prompt turn advanced; and prevent a terminal/cancelled
binding from becoming active again. The transition that persists acceptance
inserts the receipt and moves `admitted -> configured` atomically. Its
zero-send failure counterpart inserts the failure and terminalizes/cancels the
binding atomically. A
partial unique index permits only one `dispatching|awaiting_terminal` binding
per generation. The lifecycle custodian cannot authorize a prompt on behalf of
another binding, and deleting or closing the custodian does not erase the
other durable owner rows.

`provider_process_bindings` is keyed by session-generation ID and contains the
physical lineage/session custodian tuple, provider, child PID, process-group ID
where supported,
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
`configuration_attempt_index`, non-null `prompt_turn_id` FK, `work_item_id`,
provider, requested pair,
configuration state, bounded receipt JSON/digest, and
created/updated timestamps; execution, occurrence, generation/session,
nullable `continuation_id` and `steward_lane_id` FKs, accepted pair, wire pair, source digest,
verified time, and option-snapshot fields follow
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

New-generation allocation is single-flight and has exactly one owner for all
configuration-owner kinds, including Steward. The caller submits the registered
Class A `provider_configuration.reserve_new_generation` operation to the
daemon-owned `DbWriter`; its transaction closure uses the existing P061
immediate-transaction primitive.
The owner-row CAS pre-generates generation ID `g`, requires both active fields
null, reads `next_configuration_attempt_index = n`, inserts generation `g` in
pre-session state with the same lineage/owner/attempt, and writes next index
`n + 1` plus active pair `(n, g)`. The rebuilt generation table permits null
provider-session and process fields only in this pre-session state. A second
caller receives `configuration_attempt_active`; it does not skip to `n + 1` or
launch another process. The generation, launch intent, eventual process
binding, and logical generation-owner binding all carry `(owner, n, g)`.
Receipt or failure settlement requires that exact active pair. Success inserts
`(owner, n)`, moves the current receipt pointer, and clears the active pair in
one transaction. Failure appends the failure row and clears it only after
identity-safe cleanup is terminal; ambiguous cleanup leaves the pair and owner
quarantined for startup. Gaps from a transaction that committed an allocation
but crashed before launch remain valid and are never reused.

Existing-generation reuse never enters the physical generation-creation path.
It uses the separate
`provider_configuration.reserve_existing_generation` transaction defined above,
which does not create a generation but does atomically consume the configuration
owner's next attempt index while inserting the unique turn/generation binding,
derived owner receipt, and current-receipt pointer. A committed replay returns
the stored attempt; a validation/conflict loser rolls back without consuming an
index.
Crash/race fixtures cover new-vs-new, existing-vs-existing, and
existing-vs-new reservations for the same turn; one binding survives and every
loser performs zero launch/session/prompt I/O.

All new non-startup writes obey the P075 gateway. They are registered in
`control-plane/crates/db/write-operation-registry.toml`; direct pool
transactions are forbidden outside the existing startup/migration bypass. The
minimum operation registry is normative:

| Operation name | Lane / replay key | Shutdown policy |
|---|---|---|
| `provider_configuration.reserve_new_generation` | `critical_barrier`; owner kind/ID + prompt turn ID + pre-generated generation ID | deny new admission |
| `provider_configuration.reserve_existing_generation` | `critical_barrier`; owner kind/ID + turn ID + generation ID | deny new admission |
| `provider_configuration.settle_success` | `critical_barrier`; owner kind/ID + attempt index + receipt digest | admit terminal settlement |
| `provider_configuration.settle_failure` | `critical_barrier`; owner kind/ID + attempt index + failure code | admit terminal settlement |
| `provider_configuration.invalidate` | `critical_barrier`; generation + option-snapshot revision/digest | admit safety settlement |
| `provider_prompt_turn.prepare` | `critical_barrier`; prompt-turn ID + owner predicate digest | deny new admission |
| `provider_prompt_turn.settle_sent` | `critical_barrier`; prompt-turn ID + transport outcome digest | admit terminal settlement |
| `provider_prompt_turn.settle_unknown` | `critical_barrier`; prompt-turn ID + quarantine reason | admit terminal settlement |
| `provider_generation_owner.settle` | `critical_barrier`; generation + owner + turn | admit terminal settlement |
| `p079_repair.admit_or_retry` | `critical_barrier`; operation ID + attempt index | deny new admission |
| `p079_repair.settle_validation` | `critical_barrier`; operation ID + attempt index + validation-evidence digest | admit terminal settlement |
| `p086_continuation.admit` | `operator_command` for operator requests, otherwise `critical_barrier`; command journal ID | deny new admission |
| `steward_lane.claim_or_retry` | `critical_barrier`; analysis/lane + retry index | deny new admission |
| `steward_lane.settle` | `critical_barrier`; analysis/lane + terminal digest | admit terminal settlement |
| `runtime_mutation_fence.persist_fatal` | `safety_fence`; next epoch + fatal reason digest | admit safety settlement only |

Migration 100 also creates private
`class_a_operation_results_v1`, owned only by
`db::repos::write_operation_results`:

```sql
CREATE TABLE class_a_operation_results_v1 (
  operation_name TEXT NOT NULL,
  journal_key TEXT NOT NULL,
  request_schema_version TEXT NOT NULL,
  request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
  result_schema_version TEXT NOT NULL,
  result_json TEXT NOT NULL CHECK (length(result_json) <= 16384),
  result_sha256 TEXT NOT NULL CHECK (length(result_sha256) = 64),
  committed_at TEXT NOT NULL,
  PRIMARY KEY (operation_name, journal_key)
);
```

`journal_key` is the domain-separated SHA-256 of the exact replay components in
the registry row, encoded with the common length-prefixed codec; raw IDs are not
concatenated. `request_sha256` and `result_sha256` are SHA-256 over
duplicate-key-rejected RFC 8785 JSON with explicit schema/version tags. Result
JSON is private authority evidence, contains no provider-session secret or raw
provider payload, and uses the operation's closed Rust result enum. The
transaction first loads `(operation_name, journal_key)`: matching request digest
returns the stored typed result without domain writes, a different digest
returns `Conflict`, and absence permits the mutation. It inserts the result row
in the same transaction as all natural rows. Decode, schema, digest, or
natural-row mismatch is fatal evidence corruption, never `AlreadyMatching`.

The exhaustive natural-result mapping is:

| Registered operation | Result codec | Natural rows cross-checked on replay |
|---|---|---|
| both configuration reservations | `ProviderConfigurationReservationResultV1` | owner allocator/active pair, generation when new, generation-owner binding, receipt when existing |
| configuration success/failure | `ProviderConfigurationSettlementResultV1` | owner active pair/state/pointer, generation, receipt or failure row |
| configuration invalidation | `ProviderConfigurationInvalidationResultV1` | generation snapshot state, append-only invalidation, affected owner projections |
| prompt prepare/sent/unknown | `PromptTurnCasResultV1` | prompt turn plus exact owner mirror/quarantine rows |
| generation-owner settle | `GenerationOwnerSettlementResultV1` | generation binding, terminal receipt link, active owner and every collateral owner row |
| P079 admit/retry | `P079AttemptAdmissionResultV1` | operation, guarded slot, lease, work item, turn, attempt-link, and fallback child/parent link when applicable |
| P079 post-validation settle | `P079PostValidationSettlementResultV1` | repair item, lease, operation, repair event, parent execution, validated/rejected artifact evidence, and transition hold/release |
| P086 admit | `P086ContinuationAdmissionResultV1` | existing command journal, continuation, work item, turn, and provider-send side effect |
| Steward claim/retry | `StewardLaneClaimResultV1` | analysis, lane/retry counter, work item, and turn; no generation/configuration attempt |
| Steward settle | `StewardLaneSettlementResultV1` | both lane states, analysis reduction, work-item terminal state, and terminal artifacts |
| fatal mutation fence | `RuntimeMutationFenceResultV1` | singleton durable epoch/state/reason row and in-memory commit-barrier epoch |

Every codec contains its natural IDs and closed outcome; no generic string
result is accepted. The registry generator requires exactly one mapping row and
codec implementation for every operation name. Commit-before-ack reconciliation
reads this journal, verifies every listed natural row in one read transaction,
and returns the stored result. Missing journal remains `Unknown` even if some
natural rows appear suggestive; journal-with-missing/mismatched natural truth
closes the fatal admission fence.

DbWriter acknowledgement certainty and committed domain outcome are distinct
types. `DbWriterAcknowledgementV1` is closed to:

| Acknowledgement | Meaning | Retry rule |
|---|---|---|
| `committed` | Transaction committed and its journal row is readable | Return the stored operation-specific domain result |
| `rejected_before_start` | Existing `WriteBusyExhausted`, shutdown admission refusal, or queue rejection proved that no transaction started | A bounded Class A retry may reuse the identical key when policy allows |
| `failed_before_start` | Connection/validation failure with an explicit `transaction_started = false` | No reconciliation is needed; return the typed pre-start failure |
| `uncertain_after_start` | Timeout, cancellation, writer-task loss, dropped acknowledgement, or any failure without proof that the transaction did not start | Reconcile by journal key; never assume rollback |

P075 replaces phase-ambiguous `WriteFailed` at this boundary with the last two
typed cases; adapters may preserve the old error for unrelated callers but no
operation in this proposal may consume it. A committed operation stores its own
closed domain result, never `Unknown`. Prompt-turn CAS stores
`Applied|AlreadyMatching|Conflict|Missing`; reservation and settlement
operations store their named closed result enums. The transaction writes its
natural idempotency key, serialized domain result, and canonical result digest
in the same commit as the domain rows.

The caller-facing `OperationObservationV1<T>` is `Known(T)` or `Unknown`.
`committed` returns `Known(stored_result)`. On `uncertain_after_start`, the
caller may perform one immediate bounded read by the identical journal key, but
absence is not a settlement decision: it returns `Unknown` and atomically hands
the complete immutable operation envelope to daemon-owned
`ClassAReconciliationSupervisor`. The caller drops all I/O authority and the
domain owner remains held/quarantined. A different request digest is known
`Conflict`; CAS `Missing` remains a known committed domain result and is never
conflated with acknowledgement uncertainty.

The supervisor task is uncancellable by the originating request and is owned by
the daemon supervisor until process exit. It retains the writer task/journal
key, waits for writer completion when available, and polls the result journal
with bounded exponential backoff. An exact result verifies every natural row
and runs the operation's idempotent completion callback; proven transaction
rollback/no-start runs the typed failure callback. It never resubmits the
mutation or permits provider/process/prompt I/O. If neither result nor rollback
can be proved before the safety deadline, `close_first_fatal` transfers
ownership to restart; the process does not continue normal service.

Restart reconciliation scans every Class A result plus all registered
operation-specific unresolved natural-owner states before consumers open. A
late commit is therefore observed either by the same-process supervisor or by
startup, while a process death aborts an uncommitted SQLite transaction. The
generated registry provides one reconciliation callback and one startup
selector per result codec; missing either fails the gate. Faults delay commit
until after the immediate read, drop the caller and acknowledgement, stop the
supervisor before/after journal visibility, and restart. Every case converges to
one stored result with zero duplicate I/O. A post-I/O unknown terminal write
invokes `close_first_fatal` immediately rather than waiting for the ordinary
deadline.

The shutdown allowlist adds every terminal-settlement operation named above but
not admission, reserve, retry, or new-process operations. Saturation, timeout
before/after transaction start, commit-before-ack, shutdown admission, writer
crash, and restart fixtures cross every acknowledgement case with every
operation-specific result and every owner kind. They prove idempotent
convergence and that a late commit cannot create a second generation, turn, or
prompt.

All runtime mutations, not only newly added operations, move behind one
DB-owned `RuntimeMutationFenceV1` and `DbWriter`. Migration 100 creates singleton
`runtime_mutation_fence(singleton_id = 1, epoch, state, fatal_reason_sha256,
updated_at)`, where state is `open|fatal`. The db crate owns the matching
in-memory epoch plus one commit-barrier mutex. Each queued mutation captures an
open epoch, verifies it before `BEGIN IMMEDIATE`, and acquires the commit barrier
before its final epoch check and `COMMIT`.

Daemon owns exactly one `FirstFatalCoordinator`; no crate receives separate
mutation-fence or prompt-fence close authority. Its sole mutation method
`close_first_fatal(FatalServeReasonV1)` acquires the commit barrier and a
first-reason latch, ignores a later reason, increments the in-memory epoch,
closes both `RuntimeMutationFenceV1` and `PromptAdmissionFence`, and invokes the
private shutdown-proof `runtime_mutation_fence.persist_fatal` transaction while
still owning fatal linearization. That transaction CAS-persists the exact next
epoch/reason and is the sole write admitted after closure. Only after the durable
row is readable does the coordinator disable the ordinary writer queue and
publish the failed-serve watch notification. Therefore a commit holding the
barrier linearizes before fatal; `close_first_fatal` holding it first forces
every old-epoch transaction to roll back, and prompt admission closes at the
same point. There is no second compare-exchange linearization point.

If fatal persistence fails, both in-memory fences and the first-reason latch
remain closed/frozen, no watch success is claimed, and daemon exits with the
fatal bootstrap code; startup derives fatal state from unresolved authority
before opening any consumer. Clean startup reconciliation increments the
durable epoch and reopens both fences before producing
`PreflightCompleteToken`; no running process reopens them. Concurrency fixtures
race every fatal source and paused-before-commit writer, require one immutable
reason/epoch, and prove persist-before-notify ordering.

A `syn` plus SQL-call-site inventory classifies every production mutation
producer: GraphQL and MCP commands, scheduler/work-queue claim/requeue,
orchestrator/executor settlement, retry/escalation, P079, P080 recovery,
P086 continuation, Steward, projection rebuild, runtime metrics, auth reload
audit, and background cleanup. Each route names a registered operation or an
explicit startup/preflight-only transaction requiring
`PreflightLockGuard`. Runtime pools expose no public write connection and all
read-only pools set `PRAGMA query_only = ON`; a direct `pool.begin`, mutating
`sqlx::query`, or repository `_tx` call outside DbWriter/preflight owners fails
the retained gate. Fixtures pause every producer immediately before commit,
close fatal concurrently, and require rollback, no journal/natural-row change,
closed consumers, and unhealthy readiness. A mutation committed before the
fatal barrier remains valid and is never mislabeled as later.

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

`provider_configuration_failures` is append-only with primary key `id` and a
unique key on owner kind/ID plus attempt index. It stores non-null
owner kind/ID, configuration-attempt index, `prompt_turn_id`, and work item,
nullable generation/process binding,
typed failure code, optional source-acceptance digest, cleanup state
`cleanup_pending | reaped | identity_ambiguous`, and timestamps, but no accepted
pair or provider-session secret. If receipt persistence fails after provider
acceptance, the receipt transaction rolls back, the manager sends zero prompt,
and the manager closes the generation through its supervised generation actor.
It then runs a separate minimal settlement transaction that writes this failure
row, sets owner configuration to
`failed_before_prompt`, evidence to `receipt_unavailable`, leaves current receipt
null, and keeps the turn `not_started`. If even that settlement cannot commit,
the daemon enters failed-serve; startup finds the still-configuring generation,
identity-checks/reaps it, and writes the same failure before consumers open.
Neither path invents a `ProviderConfigurationReceiptV1`.

`provider_prompt_turns` has `id` as primary key; non-null `prompt_kind`,
`turn_index`, `prompt_owner_kind`, `prompt_owner_id`, `work_item_id`, `provider`,
and `transport_family`; nullable generation/session IDs, run ID, stage execution
ID, agent ID, agent execution, occurrence, captured run epoch, `mediation_record_id` FK,
`escalation_ledger_id` FK, and `steward_lane_id` FK; contract version;
nullable `p079_operation_id`, `p079_attempt_index`, and `p079_lease_key`;
`dispatch_state`;
start/sent/unknown timestamps; typed failure code; and created/updated
timestamps. Foreign keys bind execution when present and always bind the work
item. Owner kind is `invoke_agent`, `p017_mediation`, `p058_escalation`,
`p079_repair`, `p079_fallback_child`, `p086_continuation`, or
`steward_agent_lane`. A CHECK requires execution, occurrence, and run epoch for
the first six, plus run/stage/agent IDs, with null lane FK;
`p017_mediation` additionally requires a mediation FK and owner ID equal to the
mediation-owned AgentExecution ID and a null escalation FK;
`p058_escalation` requires a non-null escalation FK matching the execution's
ledger and null mediation FK. Both P079 owner kinds require non-null P079
operation/attempt/lease fields, `prompt_owner_id = p079_lease_key`, and null
mediation/escalation FKs; every non-P079 row requires all three P079 fields
null. The other run owners require both special-owner FKs null. Steward requires execution, occurrence, epoch, mediation, and
escalation FKs null plus a lane FK equal to owner ID. The exact row-level SQL
CHECK for `steward_agent_lane` requires all execution, occurrence, run-epoch,
mediation, and escalation columns to be null and
`steward_lane_id = prompt_owner_id`. SQLite
`BEFORE INSERT/UPDATE` triggers (not a cross-table CHECK) additionally require
the referenced work item to be `steward_analysis` with `run_id IS NULL` and
`stage_id IS NULL`, and require the lane's analysis, agent, generation, lineage,
and work-item IDs to match.

P079 insert/update triggers require the exact operation/attempt/lease row, kind,
parent execution, child execution when fallback, work item, occurrence,
selected binding, and frozen policy hash to match. A repair turn requires an
`OutputContractRepair` item; a fallback-child turn requires the typed
`production.p079_fallback` envelope and its child `InvokeAgent` item. Initial
fallback dispatch is allowed only through this lease-bound owner. Every
permit, cancellation, collateral, startup, and replay reducer joins these three
P079 authority columns; no fallback child is classified as ordinary merely
because its work-item kind is `InvokeAgent`.

P058 cross-row authority is structural and immutable. Migration 100 creates
append-only `p058_execution_prompt_authority` with exact columns
`agent_execution_id`, `escalation_ledger_id`, `run_id`,
`stage_execution_id`, `agent_id`, `tier_id`, `tier_kind`,
`tier_attempt_index`, and `policy_hash`. Its primary key is
`agent_execution_id`; the complete tuple is also `UNIQUE`. Reservation copies
the matching immutable P058 tier-attempt history row into this table in the same
transaction that creates the execution/work item. `BEFORE UPDATE` and
`BEFORE DELETE` triggers always abort; terminal state is recorded elsewhere.

`provider_prompt_turns` adds the four non-null P058 fields `p058_tier_id`,
`p058_tier_kind`, `p058_tier_attempt_index`, and `p058_policy_hash` for a P058
owner and nulls them for every other owner. Its P058 branch has this complete
composite foreign key:

```sql
FOREIGN KEY(
  agent_execution_id, escalation_ledger_id, run_id, stage_execution_id,
  agent_id, p058_tier_id, p058_tier_kind, p058_tier_attempt_index,
  p058_policy_hash
) REFERENCES p058_execution_prompt_authority(
  agent_execution_id, escalation_ledger_id, run_id, stage_execution_id,
  agent_id, tier_id, tier_kind, tier_attempt_index, policy_hash
)
```

Its P058 row CHECK also requires `prompt_owner_id = agent_execution_id`, a
non-null ledger, null mediation/lane IDs, and prompt kind `original`. Therefore
a ledger or tier authority that belongs to a different execution is rejected by
SQLite even when repository validation is bypassed. Insert triggers additionally
verify that the source ledger history row, execution, and work item match the
complete tuple at reservation time. Direct-SQL negative fixtures cross two valid
ledgers/executions, mutate each authority component independently, and attempt
post-reservation update/delete of tier ID, kind, attempt, policy hash, execution,
and ledger; every statement is rejected.

Partial unique indexes
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
AgentExecutions. It copy-validates and copies the already-enqueued occurrence
ID/sequence; it never invokes `TopologyOccurrenceAllocator`. P017 also binds the mediation record and P058 binds the
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
fallback child executions. Claim, status transition, startup selector, and
internal metrics handle the new kind explicitly. Existing MCP/report schemas do
not gain a work-item-kind field in this slice. Unknown kinds still fail closed.
A retained enum/SQL round-trip gate prevents a repair row from
being serialized or claimed as `invoke_agent`.

New append-only migration
`100_model_truth_prompt_authority.sql` leaves the already-applied migration 095
file and checksum byte-identical. Migration 100 rebuilds
`output_contract_repair_leases` as
`output_contract_repair_leases_v2`. Its state check is `reserved`,
`dispatch_pending`, `prompt_sent`, `dispatch_unknown`, `settled`, or
`legacy_unverified`; it adds
`repair_prompt_kind`, `dispatch_started_at`, `prompt_sent_at`, and
`dispatch_unknown_at`. `repair_prompt_kind` is
`code_writer_completion_repair` or `output_contract_repair` for a repair lease
and null for a fallback lease. New `output_contract_repair_attempt_slots`
owns guarded monotonic attempt allocation. New
`output_contract_repair_attempt_links` owns `linked_v2|legacy_unverified` plus
nullable work-item/prompt-turn FKs. Every new repair link requires its
`output_contract_repair` item and P079 turn; every new fallback link requires
the fallback InvokeAgent item and child `original` turn. Only a migrated
unverified link may have both FKs null.
`dispatch_committed_at` remains a deprecated readback alias. It is null unless
the canonical v2 row is `prompt_sent`, in which case it equals
`prompt_sent_at`; an old migration-095 value is preserved only in the upgrade
journal and may seed `dispatch_started_at`, never prompt-sent truth. Domain
enums, repository parsers, indexes,
TTL sweeps, and reference schemas change in the same release.

`output_contract_repair_operations_v1` owns logical operation ID, parent
execution/occurrence, selected repair/fallback kind, one permanently consumed
semantic budget, `max_infrastructure_attempts INTEGER NOT NULL DEFAULT 2`, next
infrastructure-attempt index, and terminal result. Each
lease is one attempt and adds non-null operation ID plus attempt index, unique
together. Creating the operation consumes exactly one selected budget; a repair
sets only `repair_budget_consumed`, a fallback only
`fallback_budget_consumed`, and the opposite flag remains false.

Migration 100 renames the lease and fallback-link tables created by immutable
migration 095 to private `*_v1_source` names, creates the following canonical
tables, copies and verifies every source row, then drops the source tables only
in the final tracked-schema swap. This is the complete v2 DDL; omitted indexes
are only non-semantic lookup indexes over the shown columns:

```sql
CREATE TABLE output_contract_repair_operations_v1 (
  operation_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'output_contract_repair_operation_v1'),
  repair_event_id TEXT NOT NULL
    REFERENCES output_contract_repair_events(repair_attempt_id),
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT NOT NULL,
  parent_agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  parent_task_occurrence_id TEXT,
  selected_kind TEXT NOT NULL CHECK (selected_kind IN ('repair','fallback')),
  repair_budget_consumed INTEGER NOT NULL CHECK (repair_budget_consumed IN (0,1)),
  fallback_budget_consumed INTEGER NOT NULL CHECK (fallback_budget_consumed IN (0,1)),
  budget_provenance TEXT NOT NULL CHECK (budget_provenance IN (
    'consumed_v2','adopted_migration_095','consumed_migration_100',
    'legacy_unverified'
  )),
  source_schema_version TEXT NOT NULL CHECK (source_schema_version IN (
    'native_v2','p079_migration_095'
  )),
  source_lease_key TEXT,
  max_infrastructure_attempts INTEGER NOT NULL DEFAULT 2
    CHECK (max_infrastructure_attempts = 2),
  next_attempt_index INTEGER NOT NULL DEFAULT 0
    CHECK (next_attempt_index >= 0 AND
           next_attempt_index <= max_infrastructure_attempts),
  operation_state TEXT NOT NULL
    CHECK (operation_state IN ('active','settled','legacy_unverified')),
  terminal_result TEXT CHECK (terminal_result IN (
    'accepted','rejected_invalid','skipped_ineligible','unavailable',
    'failed_transport','deadline_exceeded','cancelled','superseded_ignored',
    'lease_contended','budget_exhausted','policy_denied','delivery_unknown',
    'legacy_unverified'
  )),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  CHECK ((selected_kind = 'repair' AND fallback_budget_consumed = 0 AND
          (repair_budget_consumed = 1 OR
           (operation_state = 'legacy_unverified' AND
            budget_provenance = 'legacy_unverified'))) OR
         (selected_kind = 'fallback' AND repair_budget_consumed = 0 AND
          (fallback_budget_consumed = 1 OR
           (operation_state = 'legacy_unverified' AND
            budget_provenance = 'legacy_unverified')))),
  CHECK ((operation_state = 'active' AND budget_provenance IN (
           'consumed_v2','adopted_migration_095','consumed_migration_100'
         ) AND ((selected_kind = 'repair' AND repair_budget_consumed = 1) OR
                (selected_kind = 'fallback' AND fallback_budget_consumed = 1))) OR
         operation_state <> 'active'),
  CHECK ((source_schema_version = 'native_v2' AND
          budget_provenance = 'consumed_v2' AND source_lease_key IS NULL) OR
         (source_schema_version = 'p079_migration_095' AND
          source_lease_key IS NOT NULL AND
          budget_provenance IN (
            'adopted_migration_095','consumed_migration_100','legacy_unverified'
          ))),
  CHECK ((budget_provenance = 'legacy_unverified') =
         (operation_state = 'legacy_unverified')),
  CHECK ((operation_state = 'active' AND terminal_result IS NULL) OR
         (operation_state = 'settled' AND terminal_result IS NOT NULL AND
          terminal_result <> 'legacy_unverified') OR
         (operation_state = 'legacy_unverified' AND
          terminal_result = 'legacy_unverified'))
);

CREATE TABLE output_contract_repair_attempt_slots (
  operation_id TEXT NOT NULL
    REFERENCES output_contract_repair_operations_v1(operation_id),
  attempt_index INTEGER NOT NULL CHECK (attempt_index >= 0 AND attempt_index < 2),
  lease_key TEXT NOT NULL,
  created_at TEXT NOT NULL,
  PRIMARY KEY (operation_id, attempt_index),
  UNIQUE (lease_key),
  UNIQUE (operation_id, attempt_index, lease_key),
  FOREIGN KEY (lease_key) REFERENCES output_contract_repair_leases(lease_key)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TABLE output_contract_repair_leases (
  lease_key TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'output_contract_repair_leases_v2'),
  repair_event_id TEXT NOT NULL
    REFERENCES output_contract_repair_events(repair_attempt_id),
  operation_id TEXT NOT NULL
    REFERENCES output_contract_repair_operations_v1(operation_id),
  attempt_index INTEGER NOT NULL CHECK (attempt_index >= 0 AND attempt_index < 2),
  run_id TEXT NOT NULL REFERENCES runs(id),
  stage_execution_id TEXT NOT NULL,
  parent_agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  lease_kind TEXT NOT NULL CHECK (lease_kind IN ('repair','fallback')),
  lease_state TEXT NOT NULL CHECK (lease_state IN (
    'reserved','dispatch_pending','prompt_sent','dispatch_unknown','settled',
    'legacy_unverified'
  )),
  settled_result TEXT CHECK (settled_result IN (
    'accepted','rejected_invalid','skipped_ineligible','unavailable',
    'failed_transport','deadline_exceeded','cancelled','superseded_ignored',
    'lease_contended','budget_exhausted','policy_denied','delivery_unknown',
    'legacy_unverified'
  )),
  reclamation_reason TEXT CHECK (reclamation_reason IN (
    'ttl_expired_reserved','ttl_expired_dispatch_pending',
    'ttl_expired_prompt_sent','cancellation','supersession','principal_revoked'
  )),
  frozen_fallback_policy_hash TEXT,
  idempotency_token TEXT NOT NULL,
  lease_owner_principal_id TEXT NOT NULL,
  lease_acquired_at TEXT NOT NULL,
  lease_expires_at TEXT NOT NULL,
  lease_seconds INTEGER NOT NULL,
  dispatch_started_at TEXT,
  prompt_sent_at TEXT,
  dispatch_unknown_at TEXT,
  dispatch_committed_at TEXT,
  version INTEGER NOT NULL DEFAULT 0,
  infra_retry_count INTEGER NOT NULL DEFAULT 0,
  repair_prompt_kind TEXT CHECK (repair_prompt_kind IN (
    'code_writer_completion_repair','output_contract_repair'
  )),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (operation_id, attempt_index),
  UNIQUE (operation_id, attempt_index, lease_key),
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_attempt_slots(
      operation_id, attempt_index, lease_key
    ),
  FOREIGN KEY (lease_key)
    REFERENCES output_contract_repair_attempt_links(lease_key)
    DEFERRABLE INITIALLY DEFERRED,
  CHECK ((lease_kind = 'repair' AND repair_prompt_kind IS NOT NULL) OR
         (lease_kind = 'fallback' AND repair_prompt_kind IS NULL)),
  CHECK ((lease_state = 'reserved' AND dispatch_started_at IS NULL AND
          prompt_sent_at IS NULL AND dispatch_unknown_at IS NULL AND
          settled_result IS NULL) OR
         (lease_state = 'dispatch_pending' AND dispatch_started_at IS NOT NULL AND
          prompt_sent_at IS NULL AND dispatch_unknown_at IS NULL AND
          settled_result IS NULL) OR
         (lease_state = 'prompt_sent' AND dispatch_started_at IS NOT NULL AND
          prompt_sent_at IS NOT NULL AND dispatch_unknown_at IS NULL AND
          settled_result IS NULL) OR
         (lease_state = 'dispatch_unknown' AND dispatch_started_at IS NOT NULL AND
          dispatch_unknown_at IS NOT NULL AND
          settled_result = 'delivery_unknown') OR
         (lease_state = 'settled' AND settled_result IS NOT NULL AND
          settled_result <> 'legacy_unverified') OR
         (lease_state = 'legacy_unverified' AND
          dispatch_started_at IS NULL AND prompt_sent_at IS NULL AND
          dispatch_unknown_at IS NULL AND dispatch_committed_at IS NULL AND
          settled_result = 'legacy_unverified')),
  CHECK ((dispatch_committed_at IS NULL) OR
         (lease_state = 'prompt_sent' AND
          dispatch_committed_at = prompt_sent_at)),
  CHECK (prompt_sent_at IS NULL OR dispatch_started_at IS NOT NULL)
);

CREATE TABLE output_contract_repair_attempt_links (
  lease_key TEXT PRIMARY KEY
    REFERENCES output_contract_repair_leases(lease_key),
  operation_id TEXT NOT NULL,
  attempt_index INTEGER NOT NULL,
  link_state TEXT NOT NULL CHECK (link_state IN ('linked_v2','legacy_unverified')),
  work_item_id TEXT REFERENCES work_items(id),
  prompt_turn_id TEXT UNIQUE REFERENCES provider_prompt_turns(id),
  created_at TEXT NOT NULL,
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_leases(
      operation_id, attempt_index, lease_key
    ),
  CHECK ((link_state = 'linked_v2' AND
          work_item_id IS NOT NULL AND prompt_turn_id IS NOT NULL) OR
         (link_state = 'legacy_unverified' AND
          work_item_id IS NULL AND prompt_turn_id IS NULL))
);

CREATE TABLE output_contract_repair_fallback_parent_links (
  fallback_agent_execution_id TEXT PRIMARY KEY REFERENCES agent_executions(id),
  parent_failed_agent_execution_id TEXT NOT NULL REFERENCES agent_executions(id),
  repair_event_id TEXT NOT NULL
    REFERENCES output_contract_repair_events(repair_attempt_id),
  operation_id TEXT NOT NULL
    REFERENCES output_contract_repair_operations_v1(operation_id),
  attempt_index INTEGER NOT NULL,
  lease_key TEXT NOT NULL REFERENCES output_contract_repair_leases(lease_key),
  fallback_packet_hash TEXT NOT NULL,
  fallback_principal_id TEXT NOT NULL,
  fallback_principal_capability_hash TEXT NOT NULL,
  fallback_result TEXT CHECK (fallback_result IN (
    'accepted','rejected_invalid','unavailable','failed_transport',
    'deadline_exceeded','cancelled','superseded_ignored'
  )),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE (operation_id, attempt_index),
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_leases(
      operation_id, attempt_index, lease_key
    )
);

CREATE TRIGGER p079_attempt_slot_before_insert
BEFORE INSERT ON output_contract_repair_attempt_slots
BEGIN
  SELECT CASE WHEN NOT EXISTS (
    SELECT 1 FROM output_contract_repair_operations_v1 o
    WHERE o.operation_id = NEW.operation_id
      AND o.next_attempt_index = NEW.attempt_index
      AND NEW.attempt_index < o.max_infrastructure_attempts
      AND (o.operation_state = 'active' OR
           (o.operation_state = 'legacy_unverified' AND
            o.source_schema_version = 'p079_migration_095' AND
            NEW.attempt_index = 0))
  ) THEN RAISE(ABORT, 'p079_attempt_slot_not_authorized') END;
END;

CREATE TRIGGER p079_attempt_slot_advance
AFTER INSERT ON output_contract_repair_attempt_slots
BEGIN
  UPDATE output_contract_repair_operations_v1
     SET next_attempt_index = next_attempt_index + 1,
         updated_at = NEW.created_at
   WHERE operation_id = NEW.operation_id
     AND next_attempt_index = NEW.attempt_index;
  SELECT CASE WHEN changes() <> 1
    THEN RAISE(ABORT, 'p079_attempt_slot_cas_lost') END;
END;

CREATE TRIGGER p079_lease_cannot_activate_terminal_operation
BEFORE INSERT ON output_contract_repair_leases
WHEN NEW.lease_state IN ('reserved','dispatch_pending','prompt_sent')
BEGIN
  SELECT CASE WHEN NOT EXISTS (
    SELECT 1 FROM output_contract_repair_operations_v1 o
     WHERE o.operation_id = NEW.operation_id AND o.operation_state = 'active'
  ) THEN RAISE(ABORT, 'p079_active_lease_requires_active_operation') END;
END;

CREATE TRIGGER p079_lease_cannot_reactivate
BEFORE UPDATE OF lease_state ON output_contract_repair_leases
WHEN OLD.lease_state IN ('dispatch_unknown','settled','legacy_unverified')
 AND NEW.lease_state IN ('reserved','dispatch_pending','prompt_sent')
BEGIN
  SELECT RAISE(ABORT, 'p079_terminal_lease_is_immutable');
END;

CREATE TRIGGER p079_operation_terminal_guard
BEFORE UPDATE OF operation_state ON output_contract_repair_operations_v1
WHEN OLD.operation_state = 'active' AND NEW.operation_state <> 'active'
BEGIN
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM output_contract_repair_leases l
     WHERE l.operation_id = NEW.operation_id
       AND l.lease_state IN ('reserved','dispatch_pending','prompt_sent')
  ) THEN RAISE(ABORT, 'p079_operation_has_active_lease') END;
  SELECT CASE WHEN EXISTS (
    SELECT 1
      FROM output_contract_repair_attempt_slots s
      LEFT JOIN output_contract_repair_leases l
        ON l.operation_id = s.operation_id
       AND l.attempt_index = s.attempt_index
       AND l.lease_key = s.lease_key
      LEFT JOIN output_contract_repair_attempt_links a
        ON a.lease_key = s.lease_key
     WHERE s.operation_id = NEW.operation_id
       AND (l.lease_key IS NULL OR a.lease_key IS NULL)
  ) THEN RAISE(ABORT, 'p079_operation_attempt_incomplete') END;
  SELECT CASE WHEN EXISTS (
    SELECT 1
      FROM output_contract_repair_attempt_links a
      JOIN output_contract_repair_leases l ON l.lease_key = a.lease_key
      JOIN work_items w ON w.id = a.work_item_id
     WHERE l.operation_id = NEW.operation_id
       AND a.link_state = 'linked_v2'
       AND w.status NOT IN ('completed','failed','cancelled')
  ) THEN RAISE(ABORT, 'p079_operation_has_active_work_item') END;
END;
```

SQLite `BEFORE INSERT/UPDATE` triggers supply the cross-table checks that row
`CHECK` cannot express. A `linked_v2` repair attempt-link accepts only a work
item whose kind is exactly `output_contract_repair`, whose owner tuple matches
the operation, and whose turn is the matching P079 repair turn. A fallback
attempt-link accepts only its child `invoke_agent` item and that child's
`original` turn. Native-v2 operations require `linked_v2`; only a migrated
operation may use `legacy_unverified`, and an active migrated operation may
dispatch only after its link is upgraded to `linked_v2`. Operation/lease kind,
event, run, stage, parent execution, attempt index, lease key, and canonical
prompt owner ID must all agree.

An analogous `BEFORE UPDATE` variant of
`p079_lease_cannot_activate_terminal_operation` applies the same operation-state
predicate to every active lease transition. The operation terminal guard also
requires the greatest attempt's `settled_result` to equal
`NEW.terminal_result`; `legacy_unverified` operation may reference only
`legacy_unverified` leases/links, while `settled` may reference only `settled`
or `dispatch_unknown` leases. These generated clauses are byte-compared to the
checked-in migration so the abbreviated SQL above cannot drift from the final
trigger body.

The operation update guard rejects changes to identity, selected kind, either
budget flag, provenance, source schema/key, or attempt limit. It allows exactly
two update shapes: active-to-terminal with unchanged `next_attempt_index`, or
same-state `next_attempt_index = old + 1` with all other semantic fields
unchanged and an attempt-slot row at `(operation_id, old index)`. The latter is
performed only by `p079_attempt_slot_advance`; a direct increment without that
slot fails. Slot insert checks the old index and attempt cap before its guarded
CAS. The slot-to-lease and lease-to-link foreign keys are deferred, so the one
transaction has an acyclic write order while commit still rejects a missing
lease or link.

Negative fixtures attempt both work-item-kind swaps and a cross-operation
lease/turn reference directly through SQLite. Additional
direct-SQL negatives attempt an active operation with the selected budget zero,
both budgets zero or one, wrong `budget_provenance`, native provenance carrying
a source lease, migrated provenance without one, and mutation from an adopted
budget back to zero; native or active `legacy_unverified` links; missing slot,
lease, turn, or link at commit; skipped/repeated attempt indexes; manual
`next_attempt_index` increments; and every illegal lease-state/result/timestamp
combination. Every statement or transaction is rejected by the shown
constraints, deferred FKs, or triggers.

Repair admission pre-generates all IDs and writes, in exact order, operation,
attempt-0 slot, lease, typed `OutputContractRepair` work item, `not_started`
turn, and `linked_v2` attempt-link. Fallback uses
`claim_fallback_with_lease_tx`: it pre-generates child execution and turn IDs,
then in one transaction validates the parent, inserts operation, attempt-0 slot,
and lease; creates/starts the child AgentExecution and fallback InvokeAgent item
with the validated `production.p079_fallback` envelope; inserts its `original/0`
turn owned by `p079_fallback_child` with owner ID equal to the lease key; then
inserts the `linked_v2` attempt-link and fallback-parent row. Failure at any
insert or deferred commit check rolls back all rows. No committed lease can
lack its slot/link, and no link can reference a missing work item or turn.
The child may create its one initially authorized provider generation only from
this transaction's lease-bound claim. Once that generation is interrupted or
invalidated, the typed P079 owner routes through operation settlement; it never
falls through ordinary InvokeAgent fresh-session or replay policy.

Permit moves the attempt lease/turn to
`dispatch_pending` and sets only `dispatch_started_at`; successful flush plus
final CAS moves both to `prompt_sent` and sets `prompt_sent_at`; ambiguous
delivery moves both to `dispatch_unknown` and immediately runs the typed unknown
settlement; no active item survives that terminal lease.

Provider output itself is never allowed to close only the lease. After bounded
artifact validation, the registered Class A
`p079_repair.settle_validation` operation is the sole terminal reducer. Its
request carries operation/attempt/lease/item/turn IDs, the closed validation
outcome, candidate-artifact digest, validator-version digest, and canonical
result digest. In one transaction it verifies the turn is `prompt_sent`, writes
the immutable validation evidence, terminalizes the `OutputContractRepair` work
item, moves the lease to `settled`, updates the repair event, settles or holds
the parent execution and transition, commits or quarantines the candidate
artifact, and only then moves the logical operation to `settled`. The exact
matrix is:

| Validation outcome | Item / lease / operation | Parent and artifact |
|---|---|---|
| `accepted` | `completed` / `settled(accepted)` / `settled(accepted)` | atomically publish the validated artifact, mark parent output contract repaired, release only the matching transition hold |
| `rejected_invalid` | `failed` / `settled(rejected_invalid)` / `settled(rejected_invalid)` | retain parent failure, quarantine candidate by digest, keep stage/run blocked |
| `unavailable` or `failed_transport` after a sent prompt | `failed` / matching settled result / matching settled result | retain parent failure and candidate evidence, keep blocked; no infrastructure retry |
| cancellation or supersession after validation starts | `cancelled` / matching settled result / matching settled result | preserve terminal parent truth, quarantine any candidate, apply only the existing scoped cancellation/supersession reducer |

Fallback-child terminal output uses the same operation-level terminality rules
through its existing P079 fallback result reducer; it must close the child
InvokeAgent item and lease before operation settlement. Idempotent replay with
the same validation digest returns the complete stored settlement; another
digest is `Conflict`. Fault injection pauses before/after every row mutation and
commit/ack, then proves same-process and restart convergence with one artifact
publication/quarantine and no active item, lease, or operation. Database
terminal guards reject every partial direct-SQL ordering.

Budget consumption is never refunded. A TTL-expired `reserved` attempt with
turn still `not_started` terminalizes its linked item and settles that attempt
`deadline_exceeded`; the
explicit two-attempt infrastructure allowance may atomically allocate attempt
`n` only by inserting the guarded slot at the operation's current
`next_attempt_index`; its trigger advances the allocator exactly once without
consuming another logical budget.
For repair this creates a fresh repair item/turn; for fallback it uses the same
ordered atomic admission routine to create the lease, fresh child
AgentExecution, InvokeAgent item, original turn, attempt-link, and parent link.
Prior attempt/execution rows remain terminal evidence and are never reused.
Pending, sent-without-result, or unknown expiry settles the operation with
`delivery_unknown` only after terminalizing the linked item and lease, records
`ttl_expired_dispatch_pending` or
`ttl_expired_prompt_sent`, and blocks replay.

The state/nullability/budget matrix is normative:

| Lease row | Work item / turn | Budget truth | Migration/result |
|---|---|---|---|
| New repair, any active state | `OutputContractRepair` item and P079 turn non-null | Operation repair true, fallback false | Attempt mirrors its turn atomically |
| New fallback, any active state | Existing child execution, fallback InvokeAgent item, and original turn non-null | Operation repair false, fallback true | Attempt mirrors the child original turn |
| Zero-send expired attempt | Prior item/turn terminal and provably `not_started`; new attempt gets new item/turn | Same operation budget, no second consumption | Bounded next attempt or terminal deadline result |
| Migrated terminal v1, selected budget true, every mandatory identity valid | Link when uniquely provable; otherwise canonical `legacy_unverified` link with null item/turn | Preserve selected flag as `adopted_migration_095`; opposite remains zero | Lease `settled`, operation `settled`, never replayed |
| Migrated terminal v1, selected budget false, every mandatory identity valid | Canonical `legacy_unverified` link | Preserve both zero with `legacy_unverified`; consume nothing | Lease/operation `legacy_unverified`, never replayed |
| Any migrated row with a dangling mandatory execution/event/run/stage/lease FK | No canonical operation, slot, lease, or link | Preserve source values in quarantine without FKs | Active source blocks startup; terminal source is diagnostic-only |
| Active v1 `reserved`, unique owner/kind, budget false | Create/bind one `not_started` turn in the same transaction | Atomically set the compatibility event's selected flag and operation flag to one with `consumed_migration_100`; if unavailable settle `budget_exhausted` | Remain `reserved` only after both commits |
| Active v1 `reserved`, unique owner/kind, budget true | Create/bind one `not_started` turn | Preserve selected flag as `adopted_migration_095`; opposite remains zero | Remain `reserved` |
| Active v1 `prompt_sent` or old send-side effect with validated canonical owner | Bind a turn only with unique owner | Selected true becomes `adopted_migration_095`; selected false cannot enter canonical active tables | Lease `dispatch_unknown` with old committed time copied only to `dispatch_started_at`; selected-false source goes to migration quarantine |
| Any active row with ambiguous/dangling owner, kind, turn, or contradictory budget flags | No canonical attempt/link | Preserve exact typed source envelope only in P079 migration quarantine | No canonical lease/operation; active source blocks startup, terminal source remains diagnostic-only |

Migration 095 did not enforce every execution/event relationship, so preserving
source rows does not mean forcing them through new foreign keys. Migration 100
creates append-only `p079_migration_quarantine_v1` with primary key
`(upgrade_id, source_table, source_primary_key)`, source row count/ordinal,
duplicate-key-rejected `sqlite_typed_row_v1` JSON, its SHA-256, closed reason
`dangling_run|dangling_stage|dangling_parent_execution|dangling_repair_event|dangling_lease|ambiguous_owner|contradictory_budget`, nullable parsed correlation IDs, active/terminal source classification, and timestamp. The typed envelope records every source column in declaration order as
`{name, sqlite_type, value}`; text bytes are UTF-8, integers use canonical
decimal, null is tagged, and blobs are base64. The quarantine table has no FK to
any potentially dangling source identity. It is writeable only by
`ProviderTruthUpgradeCoordinator` while holding `PreflightLockGuard`.

Only rows whose mandatory run, stage, event, parent execution, lease, and
fallback-child relationships all validate may create canonical operation,
slot, lease, link, or fallback-parent rows. Every other source row is copied to
quarantine and digest-verified before the source table can be dropped. Final
accounting requires, per source table,
`source_count = canonical_source_count + quarantine_count`, disjoint source
keys, and byte-equal source-envelope digests. An active quarantined row keeps
bootstrap failed with its sanitized reason; a terminal quarantined row is
retained for diagnostics but is never exposed to dispatch/replay selectors.
`foreign_key_check` must be empty because quarantine intentionally carries no
domain FKs. Crash/restart fixtures stop before and after each quarantine write,
canonical write, count/digest checkpoint, source swap, and final FK check.

The lease row itself is the repair-attempt-to-parent relation; the schema
installed by immutable migration 095 has no separate repair-attempt parent-link
table and migration 100 does not invent one. The fallback table's existing primary key is
`fallback_agent_execution_id`; there is no separate link ID to preserve. Its
v2 row adds non-null `operation_id` and `attempt_index`; parent identity is
unique only on `(operation_id, attempt_index)`, never globally on
`repair_event_id` or `parent_failed_agent_execution_id`. The fallback child ID
remains globally unique. Each new attempt therefore points to its own work
item, turn, lease, and child when applicable while preserving one logical
parent/budget.

For rows that pass mandatory-identity validation, migration 100's mapping from
the migration-095 source schema is exact; all others use the quarantine contract
above:

| v1 source | v2 target |
|---|---|
| validated lease `lease_key` | same lease PK; deterministic operation ID `p079_operation_v1:<sha256>` over that exact key; `attempt_index = 0`; operation `source_lease_key` is the exact key |
| lease `schema_version` | literal `output_contract_repair_leases_v2`; old value retained in the upgrade journal |
| lease event/run/stage/parent/kind | byte-for-byte same lease columns and same operation columns |
| event budget flags | Exactly selected true/opposite false becomes `adopted_migration_095`; exactly both false becomes `consumed_migration_100` only for an eligible active reserved row and atomically changes the compatibility event selected flag to one; every other zero or contradictory source tuple becomes canonical both-zero `legacy_unverified`, with raw flags preserved in the upgrade journal/evidence |
| lease state `reserved` | v2 `reserved` only after owner/classifier proof; valid-but-unreplayable zero-send evidence becomes canonical `legacy_unverified`; dangling/ambiguous identity goes only to migration quarantine |
| lease state `prompt_sent` | v2 `dispatch_unknown`; old `dispatch_committed_at` becomes `dispatch_started_at`, canonical `dispatch_committed_at` and `prompt_sent_at` are null, and the old value remains in the upgrade journal |
| lease state `settled` | a validated identity becomes canonical `settled`; selected budget false becomes canonical `legacy_unverified` only when every mandatory FK validates, otherwise quarantine; preserve result, reclamation, version, retry count, principal, policy hash, token, TTL, and timestamps |
| fallback child PK/parent/event/lease | byte-for-byte same fields; add the operation derived from its lease and `attempt_index = 0` |
| fallback packet/principal/capability/result/timestamps | byte-for-byte same fields |

One operation is created per distinct validated v1 lease; quarantined leases
create no canonical operation. Two valid legacy leases are never merged merely
because they share a parent. The upgrader stages rows by
`lease_key`, records source row count and canonical column digest, copies in
bounded key order, and checkpoints after each batch. Restart repeats inserts by
natural key and compares every copied column. Finalization requires the exact
canonical-plus-quarantine accounting/digests and complete validated
fallback-to-lease mapping before the tracked-schema swap. No source row is
dropped to satisfy a v2 key: it is represented either by one canonical source
key or one quarantine source key, never both.

Migration maps terminal validated v1 leases to `settled` or
`legacy_unverified` without inventing prompt truth. It creates one logical
operation plus attempt-0 row per distinct validated v1 lease key, moves the
preserved budget flags to the operation, and never merges two legacy leases
merely because they share a parent execution. Invalid mandatory identity is
quarantine-only.
For every active v1 lease, the upgrader queries
`artifact_source_generation_claims` by exact run ID, stage-execution ID,
`agent_execution_id = parent_agent_execution_id`, and an existing source work
item. Exactly one distinct `source_work_item_id` is required. A unique match
sets `linked_v2`. `P079PromptKindClassifierV1` then derives the planned kind
from immutable run-plan/output-contract inputs plus the repair event; an
existing runtime receipt, when present, must agree. Only a unique classification
plus the atomic budget transition above allows an active `reserved` lease to
remain reserved. With valid mandatory FKs but missing/contradictory prompt
classification or zero/multiple work-item matches, the canonical lease and
operation become terminal `legacy_unverified`, no prompt turn is created, and
an owner quarantine blocks replay. A dangling mandatory FK is quarantine-only
and cannot enter this classifier.

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

Migration 100 creates append-only `provider_session_resume_contexts` with
primary key `context_id`, literal schema version
`provider_session_resume_context_v1`, unique source-generation FK, internal
provider-session-row FK, provider, adapter-contract version, target-binding
fingerprint, canonical cwd, duplicate-free ordered additional-directories JSON,
immutable MCP descriptor-set reference/digest, canonical context JSON/digest,
and creation time. Context JSON contains the provider-session row reference but
not the provider-session secret or expanded broker secrets. Insert triggers
recompute the canonical digest and verify every referenced row; update/delete
are rejected. `agent_work_continuations` adds nullable
`provider_session_resume_context_id` plus digest. Resurrection admission requires
both non-null and matching; live-handle mode requires both null; output-only
requires null until its one-way resurrection conversion atomically installs the
pair. Cross-generation, cross-provider, changed-root, changed-MCP, and
current-workspace recomputation attempts fail before launch.

P086 admission pre-generates the continuation and ProcessContinuation work-item
IDs. One registered Class A `p086_continuation.admit` DbWriter operation performs
command idempotency and policy checks and inserts the command-journal row,
continuation, Pending work item, allocated `not_started` turn, and
`provider_send` side-effect row in `reserved`. The work item stores non-null
run/stage owner fields. A transaction-body error rolls back all five records;
an acknowledged accepted response therefore always names a claimable work item
and turn. A timeout after transaction start returns `Unknown` and reconciles by
the command-journal ID before any response or claim. Identical command replay
returns the committed IDs and cannot enqueue a second item.

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
derives/acquires the database singleton lock and returns the closed
`RuntimeDatabaseBootstrapOutcomeV1` union:

| Outcome | Owned value | Serve transition |
|---|---|---|
| `ready` | `RuntimeDatabase { pool, preflight_lock_guard }` | `starting -> normal` only after every preflight proof succeeds |
| `failed` | `FailedBootstrapOwner { preflight_lock_guard, sanitized_failure, failure_code }` | `starting -> failed`; no writable/readable runtime pool exists |

Both owner values are non-`Clone` and non-serializable. In particular,
`FailedBootstrapOwner` retains the live `PreflightLockGuard` until process exit,
so a failed serve process cannot accidentally release singleton ownership and
race another local opener. Daemon `supervisor` never acquires a second database
lock and never retries bootstrap in-process. Recovery requires a clean process
restart.

Inside that API, `run_preflight_with_guard(&mut PreflightLockGuard)` returns a
private `PreflightCompleteToken` only after migration, Rust finalization, and
reconciliation succeed. `create_pool_after_preflight(database_url, token)`
consumes that token and opens the runtime pool without calling preflight or
reacquiring the lock. The ordinary `create_pool` remains only for in-memory
tests and explicitly feature-gated maintenance binaries; a retained production
call-site scan rejects it in daemon startup. One-shot admin commands use the
same `open_runtime_database` path and keep the returned ready or failed owner
until exit.

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
precision. New receipt, acceptance, invalidation, turn, and topology-order timestamps
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
  "option_snapshot_revision": 3,
  "option_snapshot_sha256": "...",
  "verified_at": "2026-08-30T12:34:56.789Z"
}
```

Every key is required and non-null. `option_snapshot_revision` is a
non-negative integer; all digest-shaped fields are lowercase 64-character hex,
`verified_at` is `CanonicalUtcTimestampV1`, and unknown keys are rejected.

`accepted_model` and `accepted_effort` are canonical catalog values and equal
the requested exact pair after verification. Wire-value fields preserve the
exact option values selected and returned by provider `currentValue`; UI uses
canonical values and never substitutes display names or wire values.

`ProviderConfigurationReceiptV1` is owner-scoped and has exactly these
keys: `schema_version` with literal value
`provider_configuration_receipt_v1`,
`provider_configuration_contract_version`, `configuration_owner_kind`,
`configuration_owner_id`, `configuration_attempt_index`, `prompt_turn_id`,
nullable `agent_execution_id`, nullable `task_occurrence_id`, nullable
`continuation_id`, `work_item_id`, non-null `session_generation_id`, non-null
`provider_session_id`, `provider`, non-null `binding_fingerprint_sha256`,
`requested_model`, `requested_effort`, non-null `accepted_model`, non-null
`accepted_effort`, non-null `accepted_model_wire_value`, non-null
`accepted_effort_wire_value`, non-null `option_snapshot_revision`, non-null
`option_snapshot_sha256`, literal `configuration_state = configured`, non-null
`acceptance_source`, non-null `source_generation_acceptance_sha256`, non-null
`verified_at`, and the non-negative integer
`prompt_dispatch_count_at_receipt`.

`configuration_attempt_index` and `prompt_dispatch_count_at_receipt` are
non-negative integers; the former must equal the owner's allocated attempt.
`prompt_turn_id` is the exact globally unique generation-owner binding turn and
must be `not_started` when the receipt is created.

Receipt owner kind is closed to `agent_execution`, `p086_continuation`, or
`steward_agent_lane`;
acceptance source is closed to `fresh_negotiation`,
`reused_session_generation`, or `attached_session_reverification`. A
configuring, failed-before-acceptance, cancelled-before-acceptance, or legacy
projection has no receipt. It uses the append-only failure row when terminal.
Receipt is successful response-verified acceptance authority only; invalidation
is separate append-only evidence and never rewrites the receipt. These domains
are JSON Schema enums, not free strings.

All accepted/source, session, digest, option snapshot, and verification fields
are non-null, equal referenced generation acceptance, and the prompt count is
zero. No `failure_code` key exists and no unknown JSON key is accepted. For
owner kind `agent_execution`, both
execution/occurrence fields are non-null, continuation is null, and the tuple
matches the owning execution row. For `p086_continuation`, continuation,
execution, and occurrence fields are all non-null and match the continuation's
target tuple; owner ID equals continuation ID and the target execution's receipt
pointer is unchanged. For `steward_agent_lane`, execution, occurrence, and
continuation are null and the owner ID is exactly the durable lane ID whose
analysis, agent, provider, and work item match the invocation.
All receipt work-item and requested values must equal owner truth; all
configured generation fields must equal the referenced generation acceptance.

The mapping from prompt owner to configuration owner is closed and checked by
every reservation and permit:

| Prompt owner kind | Configuration owner | Required join |
|---|---|---|
| `invoke_agent`, `p017_mediation`, `p058_escalation` | `agent_execution` | The prompt's exact execution/occurrence; P017/P058 special authority also matches |
| `p079_repair` | `agent_execution` | A new configuration attempt/receipt on the operation's parent execution; operation, lease, work item, turn, generation, and parent all match |
| `p079_fallback_child` | `agent_execution` | The lease-bound child execution/occurrence and typed fallback provenance all match |
| `p086_continuation` | `p086_continuation` | Continuation, target execution/occurrence, attach receipt, work item, and generation all match; target receipt pointer is unchanged |
| `steward_agent_lane` | `steward_agent_lane` | Lane, analysis, agent, work item, lineage, and generation all match with null run/execution fields |

P079 repair and P086 may reserve only an existing/attached generation and fail
closed on mismatch; they never convert a missing receipt into an ordinary fresh
session. The table is generated into permit predicates and the owner-matrix
fixture, so adding a prompt owner without a configuration-owner mapping is a
compile/gate failure.

`ProviderConfigurationInvalidationV1` is separate append-only evidence with
required keys `schema_version = provider_configuration_invalidation_v1`,
`session_generation_id`, `prior_option_snapshot_revision`,
`prior_option_snapshot_sha256`, `invalidating_option_snapshot_revision`,
`invalidating_option_snapshot_sha256`, `observation_phase`, `reason`,
`prompt_byte_certainty`, and `observed_at`. Closed phases are
`before_prompt|during_write|awaiting_terminal|after_terminal`; reasons are
`changed|malformed|missing_required_option|contradictory`; byte certainty is
`zero|some|unknown|terminal`. It contains no provider-session secret or raw
notification bytes. The generation/owner projection joins the greatest valid
revision and never rewrites the original acceptance/receipt JSON.

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

`AcpRuntimeReceipt` deliberately remains integer `schema_version = 1`. Its Rust
type, encoder, decoder, `receipt_json` bytes, and complete nested field set do
not gain provider-configuration or prompt-turn members in this slice. Unknown
or unsupported runtime-receipt versions retain their current behavior. This is
required because existing `reports.get` and adjacent MCP/report projections
parse and expose `agent_execution_runtime_receipts.receipt_json`; adding a v2
member there would change the public payload despite an unchanged tool schema.

Configuration and dispatch correlation is relational instead. Migration 100
copies every historical `receipt_json` byte-for-byte while adding nullable
`prompt_turn_id` and non-null `prompt_link_state` columns beside it.
`provider_configuration_receipts`, `provider_configuration_failures`, and
`provider_prompt_turns` hold the new authority. The existing receipt upsert
continues to accept/encode only v1; a new private authority upsert accepts the
same frozen v1 JSON plus the already-validated relational turn/link tuple and
writes both atomically. GraphQL resolves accepted truth through those joins and
never by looking for private fields inside `receipt_json`.

The implementation adds normative `additionalProperties: false` schemas only
at `docs/reference/schemas/provider-configuration-acceptance-v1.schema.json`
and `docs/reference/schemas/provider-configuration-receipt-v1.schema.json`,
plus valid/invalid fixtures. Configuration
failure remains the typed relational row defined above rather than a nested
runtime-receipt JSON object. For run agents, the execution-row projection,
owner-scoped configuration receipt, authoritative prompt turn, and terminal
runtime-receipt relational link must agree on execution, occurrence,
turn/owner tuple, requested/accepted pair, source digest, generation, and
provider-session binding. For Steward, the owner-scoped receipt and prompt turn
must agree on analysis/lane/agent owner, work item, requested pair, generation,
and provider session; no execution projection or runtime-receipt JSON is
invented. The durable prompt-turn row remains dispatch authority; terminal
receipt linkage can confirm post-I/O evidence but never mutate dispatch state.
`ProviderConfigurationAuthority` performs the database-backed source-generation
digest comparison before any readback projection.

A retained compatibility fixture inserts the same v1 runtime receipt before
and after migration 100 and byte-compares `receipt_json`, `reports.get`, and the
adjacent existing report projection for Operator, Agent, and Observer callers.
Only the private relational columns and additive GraphQL truth may differ.

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

- `PromptDispatchAuthorityPort` exposes configuration settlement, the
  initial/final turn CAS, terminal runtime-receipt persistence, and one closed
  `settle_terminal_generation` operation covering the active owner and any
  generation-closure collateral; engine's
  `DurablePromptDispatchAuthority` implements it using `db` and is injected
  into the manager; and
- `ProviderRuntimeControlPort` exposes generation-scoped cancel/interrupt/reap
  commands without DB types; `AcpRuntimeManager` implements it and engine injects
  that port into `DispatchInvalidationCoordinator`. Its sole mutating method,
  `request_invalidation`, sends a typed command to the generation actor and
  awaits `SettledGenerationClosureOutcomeV1`; it never returns a raw handle or
  an unsettled collateral list.

Thus `acp` never depends on engine/db, while engine's existing dependency on
`acp` is sufficient for both trait definitions and runtime control. Engine's
`ProviderInvocationCoordinator` is the sole service exposed to executors. It
calls the manager's authorized operation; the manager may call only the injected
authority port for permit/turn CAS and returns a closed
`ProviderRuntimeOutcomeV1` containing phase, byte certainty, generation/process
identity, terminal owner-settlement result, cleanup result, and sanitized error.
The manager remains the sole terminal owner for an admitted provider turn: it
holds the non-cloneable generation guard through provider terminal response,
calls the authority port to persist the terminal receipt and settle the active
owner plus any typed collateral list,
performs bounded process cleanup when required, and only then releases the
guard and returns. `settle_terminal_generation` receives a complete typed settlement
command and cannot call back into ACP. The coordinator creates the command and
initiates cancellation intent before dispatch, but after manager admission it
only consumes the already-settled outcome and updates non-authoritative
presentation/metrics. It performs no second owner settlement or cleanup call.
There is no manager-to-coordinator callback, re-entrant cleanup call, or DB type
in `acp`.

Each generation actor serializes authorized prompt and invalidation commands.
If a prompt task is active, invalidation is observed through its owner/generation
tokens and that same task performs terminal settlement. If the generation is
idle, the actor acquires the gate, identity-checks/closes the process, calls
`settle_terminal_generation` for all affected not-started bindings, and then
answers the control request. Thus the coordinator can request and await
invalidation but can never run between runtime closure and authority settlement.

Daemon is the composition root: it creates the durable authority, exactly one
manager, the invocation/invalidation coordinator using the manager's
runtime-control port, and exactly one `FirstFatalCoordinator`. The coordinator
owns pre-admission DB cancellation/epoch transitions;
the admitted-turn terminal transition is owned by the manager through the
authority port as defined above. The
manager owns tokens, handles, process groups, and transport. Neither port
exposes a raw session handle or permits ACP to mutate DB directly. If accepted
truth persistence and the minimal failure settlement both fail, authority calls
`FirstFatalCoordinator::close_first_fatal` before returning. The same rule covers every prompt-authority
double failure after I/O: if transport write/flush may have succeeded, final
`prompt_sent` CAS fails, and the separate `dispatch_unknown`/quarantine
settlement also fails, authority must publish
`FatalServeReason::PromptAuthorityUnsettledAfterIo` with the sanitized
owner/turn/process tuple. Returning an ordinary owner error is forbidden.
No authority object exposes the underlying mutation fence, prompt fence, latch,
or watch sender, and there is no separate `FatalServeState` compare-exchange.
Every configuration reservation, turn prepare, work claim, and immediate
pre-write permit check reads the coordinator-owned fence. The failed-state watch
is published only after `close_first_fatal` durably stores the first reason as
defined above.

Before database bootstrap begins, daemon loads the live reloadable principal
table, constructs one Axum router, binds one listener, and starts it with
`RuntimeServeLifecycleV1 = starting`. The lifecycle is the closed one-way state
machine `starting -> normal | failed` and `normal -> failed`; `failed` is
terminal for the process. In `starting`, the outer middleware bypasses all
normal resolvers and permits only unauthenticated `/health` and `/ready`
returning 503, the exact authenticated GraphQL `daemonStatus` operation, typed
MCP refusal, and sanitized 503 for every other route. On
`RuntimeDatabaseBootstrapOutcomeV1::ready`, daemon installs the runtime owner
and atomically publishes `normal`. On `failed`, it stores the entire
`FailedBootstrapOwner` beside the server task and publishes the sanitized
failure without opening any consumer.

In `normal`, existing GraphQL/MCP routes and consumers run. A
`FirstFatalCoordinator` watch notification moves the same router to `failed`,
closes scheduler/continuation/Steward consumers and normal GraphQL
subscriptions, rejects in-flight/new mutations, signals all generation
lifecycle tokens, and performs bounded identity-safe process cleanup. The
`starting` and `failed` minimal branches use only lifecycle evidence and the
live reloadable principal table; they perform zero DB accesses. There is no
listener transfer, second bind, router replacement, or attempt to reuse a
listener consumed by `serve`. If the server task exits, daemon exits; neither a
bootstrap failure nor a runtime fatal can return to `starting` or `normal`
without a clean restart.

The `starting`/`failed` GraphQL exception is an AST whitelist, never a lexical
`contains("daemonStatus")` filter. The duplicate-key-rejected HTTP JSON object
must contain exactly `query` and `operationName`, with
`operationName = "DaemonStatus"`. The document must contain exactly one named
query operation `DaemonStatus`, no variables, extensions, fragments, aliases,
arguments, directives, inline fragments, mutation, or subscription, and this
exact root selection:

```graphql
query DaemonStatus {
  daemonStatus {
    state schemaVersion binarySchemaVersion buildSha startedAt
    lastStateChangeAt restartCountSinceBoot pid json
  }
}
```

The minimal handler parses JSON and GraphQL with the same parser/version as the
normal server, canonicalizes only insignificant whitespace/comma placement,
and compares operation kind, name, field names, and tree shape to this AST.
Malformed bodies, another operation name, missing/extra/duplicate field, mixed
`daemonStatus` plus any other root field, alias, fragment, directive, variable,
or batch are the typed failed-serve refusal. A live Operator principal is
required; Agent/Observer and revoked/disabled/re-scoped principals receive the
normal authorization refusal without diagnostics. The resolver is a dedicated
projection over in-memory lifecycle evidence and has no schema/database handle.
Starting/failed behavior tests inject a DB-access tripwire, cover every rejected
AST mutation above, and assert the counter remains zero for both accepted and
refused requests.

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
| `original` | P058 escalation execution attempt ID | Execution/occurrence, immutable P058 execution/ledger/tier/policy authority, and InvokeAgent item all match | Mark attempt/item failed; pause ledger with `provider_prompt_delivery_unknown`; terminalize tier retry authority; block stage/run without advancing tier |
| `code_writer_completion_repair` | P079 lease key | Parent execution/occurrence match; lease and its linked `OutputContractRepair` item are active | Mirror lease unknown; fail the repair item and parent execution only when still running; block stage/run |
| `output_contract_repair` | P079 lease key | Same P079 owner proof and linked `OutputContractRepair` work-item kind with generic repair event kind | Same P079 unknown settlement |
| `original` | P079 fallback lease key | Lease, operation/attempt, typed fallback envelope, parent and child executions, child occurrence/binding, and linked fallback InvokeAgent item all match | Mirror lease unknown; fail only the child execution/item, preserve parent execution and consumed operation budget, and block the current recovery path |
| `work_continuation_live_handle` | P086 continuation ID | Target execution/occurrence match even if execution is terminal; ProcessContinuation item running; continuation active and not cancelling | Mark continuation `needs_continuation_reconciliation`, fail item, preserve terminal parent execution, block stage/run |
| `work_continuation_resurrection` | P086 continuation ID | Same as live handle plus successful target-bound attach receipt | Same P086 settlement and close attached generation |
| `work_continuation_output_only` | P086 continuation ID | Frozen output-only mode, selected attachment proof, ProcessContinuation item running, source-edit prohibition frozen | Same P086 settlement; retain output-only evidence |
| `steward_analysis`, initial | Steward lane ID | Matching StewardAnalysis item and lane are active; invocation carries the same analysis, lane, agent, provider, and work item; turn `0` is active, the consumed counter is `0`, and no earlier turn exists | Mark only the lane `prompt_delivery_unknown`, then apply the sole Steward reducer: system unknown skips auditor and yields `Failed/Failed`; auditor unknown after valid system yields `Inconclusive/Completed`; forbid automatic replay |
| `steward_analysis`, sole retry | Steward lane ID | Same owner tuple; consumed counter is `1`; active turn is `1`; exactly one earlier turn `0` is terminal zero-send evidence with `not_started`, typed failure, no receipt/unknown side effect, and identity-safe cleanup | Same unknown settlement; the prior turn remains immutable and no second retry is possible |

For both P079 rows, `prompt_owner_id` is exactly the attempt's `lease_key` in
DDL, `PromptDispatchOwnerV1`, turn-ID hashing, runtime control, cancellation,
collateral reduction, startup reconciliation, and GraphQL readback.
`p079_operation_id` groups bounded attempts but is never a prompt owner ID;
`p079_attempt_index` distinguishes the attempt inside that operation. The
generic InvokeAgent row explicitly excludes `p079_fallback_child` even though
its work-item kind is `InvokeAgent`. Generated exhaustiveness tests require one
and only one row for each of the seven `ProviderPromptOwnerKind` values.

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
current owner token. Timeout/cancellation records the exact last phase; the
manager performs identity-safe cleanup and invokes authority zero-send
settlement before returning, under one additional absolute 10-second cleanup
deadline. An identity-ambiguous child is
quarantined rather than signalled. No broker request, authority call, settlement
await, transport task, or cleanup task may detach or outlive its owning
deadline. Configuration settlement uses a CAS over captured owner truth. Prompt
dispatch then holds the gate from permit through its separate fixed 10-second
transport write/flush deadline and final CAS. A committed `prompt_sent` turn
does not release the gate: the generation-owner binding moves to
`awaiting_terminal`, and the same non-cloneable guard remains held through the
provider terminal response, terminal runtime-receipt persistence, and owner
settlement. The existing execution watchdog bounds that response phase. If it
fires, cancellation closes or interrupts the generation and settles the owner
before releasing the guard. Any final settlement/cleanup await is bounded by
the execution watchdog or the explicit cleanup deadline and cannot hang daemon
shutdown. A second logical owner may be durably admitted and configured, but it
cannot receive a permit or write transport bytes while another binding is
`dispatching|awaiting_terminal`.

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
delivers the closed result `owner_interrupted | generation_closed` with the
exact collateral owner/turn list to the manager task holding or acquiring the
gate. That task calls `settle_terminal_generation`; only after durable
settlement does the manager remove a live handle and return the settled result
to the coordinator.

If the provider cannot interrupt one request without closing the shared
generation, `generation_closed` is not silently treated as targeted success.
The manager's one terminal authority call atomically settles the cancelled
owner by byte certainty and reduces every collateral binding through this
closed matrix. `session/new`
counts include only automatic recovery caused by the closure:

| Collateral owner with `not_started` turn | Settlement | Automatic `session/new` / replay |
|---|---|---|
| Ordinary InvokeAgent original | Keep turn `not_started`; invalidate its old receipt and allocate a fresh checked generation only when the frozen workflow/session policy permits | At most one; prompt remains subject to a new owner permit |
| P017 mediation original | Fail the current execution/item zero-send and invoke only the durable P017 attempt reducer | No transparent rebind; a later fresh execution exists only when P017 retry authority allocates it |
| P058 escalation original | Fail the current tier attempt zero-send and invoke only the P058 tier reducer | No transparent rebind; a later fresh execution exists only through its tier-attempt authority |
| P079 repair | Settle attempt `unavailable`, fail the linked `OutputContractRepair` item, preserve operation budget and parent execution, and block for existing P079 recovery policy | Zero; parent-generation requirement forbids fresh fallback and automatic replay |
| P079 fallback child | Settle the exact fallback attempt `unavailable`, fail only its lease-bound child execution/item, preserve the operation/parent and consumed fallback budget | Zero; typed `p079_fallback_child` ownership forbids transparent fresh session, attach, or replay |
| P086 continuation, any mode | Settle continuation `needs_continuation_reconciliation`, fail ProcessContinuation item, and preserve the target execution | Zero; no automatic attach, resurrection, fresh session, or prompt replay |
| Steward lane | Consume the one zero-send retry only when every Steward retry predicate still holds; otherwise apply the sole Steward terminal reducer | At most one across the lane's durable `max_zero_send_retries = 1` budget |

A collateral `prompt_sent` binding remains sent and receives
`provider_generation_interrupted_by_scoped_cancel` for its owner-specific output
recovery; it is never resent. A collateral `dispatch_pending` binding is
impossible because `SessionPromptGate` permits one active dispatch and remains
held through terminal settlement. No collateral owner becomes
`dispatch_unknown` merely because another owner was cancelled. The matrix is
exhaustive over owner kind, turn state, provider interruption result, frozen
fresh-session policy, and remaining owner retry authority.
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
its own bytes; B keeps sent truth or applies the exact owner-kind collateral row
to not-started truth, never inherits A's failure, and never becomes unrelated
`dispatch_unknown`. Only the ordinary-owner row may allocate a fresh
generation. Run-wide and fatal generation cancellation still settle every owner
through the generation token.

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

These are committed `PromptTurnCasResultV1` values inside
`OperationObservationV1`; `Missing` is `Known(Missing)`. A DbWriter
`uncertain_after_start` with no readable journal is the separate
`OperationObservationV1::Unknown` and never appears in this table.

Fault injection covers a provider whose flush completes, final CAS fails, and
unknown/quarantine settlement fails independently. It proves the fatal channel
notification is published before the invocation future returns, readiness
becomes unhealthy,
the coordinator closes admission and durably stores first-fatal before the guard
can admit another owner, the already-bound tri-state router exposes only failed
routes, normal consumers/subscriptions close, no later mutation commits, the
identity-matched child is reaped, and restart writes one unknown settlement
before reopening. The same fixture runs for original, P017, P058, both P079
kinds, all P086 modes, and Steward.

Crash/restart behavior is frozen at each durable boundary:

| Last durable boundary | Startup action | Prompt replay |
|---|---|---|
| Owner/turn reserved; no launch intent | Settle typed preparation failure; an original owner may use only its frozen workflow retry budget, while Steward uses `max_zero_send_retries = 1` and its durable consumed counter | Only through a new checked claim with a fresh turn |
| `spawn_pending`; launch barrier not released | Observe barrier EOF/child absence, settle zero-send launch failure | Same zero-send policy |
| PID/start identity persisted; barrier released; no correlated `session/new` or P086 `session/resume` result | Identity-check and reap child, settle configuration failure | No reuse of old generation |
| `session/new`, P086 `session/resume`, or configuration `configuring`; turn `not_started` | Identity-check and reap, write `failed_before_prompt`; ambiguous identity quarantines owner | No P079/P086 fresh fallback |
| Configured receipt committed; turn `not_started`; daemon lost transport | Reap old generation; an ordinary owner may use only its frozen recovery policy, Steward may retry only through a new turn under its one-retry lane authority, and P079/P086 fail closed | Never reuse old receipt as new-generation truth |
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
  configuring configured invalidated_after_acceptance failed_before_prompt
  cancelled_before_prompt legacy_unverified
}
enum ProviderPromptDispatchState {
  not_started dispatch_pending prompt_sent dispatch_unknown
}
enum ProviderPromptKind {
  original code_writer_completion_repair output_contract_repair
  work_continuation_live_handle work_continuation_resurrection
  work_continuation_output_only steward_analysis
}
enum ProviderPromptOwnerKind {
  invoke_agent p017_mediation p058_escalation p079_repair
  p079_fallback_child p086_continuation steward_agent_lane
}
enum ProviderConfigurationOwnerKind {
  agent_execution p086_continuation steward_agent_lane
}
enum ProviderConfigurationAcceptanceSource {
  fresh_negotiation reused_session_generation attached_session_reverification
}
enum RuntimeReceiptLinkState {
  linked_v2 legacy_pre_prompt legacy_unverified
}
enum ProviderConfigurationEvidenceState {
  pending receipt_available invalidated receipt_unavailable not_applicable
  legacy_unverified
}
enum ProviderPromptDeliveryTruth {
  not_started original_pending original_sent repair_pending repair_sent
  continuation_pending continuation_sent steward_pending steward_sent unknown
  legacy_unverified
}
enum TimelineLaneKind {
  occurrence run_events
}
enum TimelineIdentityState {
  matched_occurrence_v2 unassociated_run_event
}
type ProviderPromptConfigurationTruth {
  schemaVersion: String!
  configurationOwnerKind: ProviderConfigurationOwnerKind
  configurationOwnerId: ID
  configurationAttemptIndex: Int
  agentExecutionId: ID
  taskOccurrenceId: ID
  continuationId: ID
  sessionGenerationId: ID
  provider: String
  requestedModel: String
  requestedEffort: String
  acceptedModel: String
  acceptedEffort: String
  providerConfigurationState: ProviderConfigurationState
  configurationEvidenceState: ProviderConfigurationEvidenceState!
  acceptanceSource: ProviderConfigurationAcceptanceSource
  providerConfigurationVerifiedAt: String
  providerConfigurationInvalidatedAt: String
  invalidatingOptionSnapshotRevision: Int
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
  configurationTruth: ProviderPromptConfigurationTruth!
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
  providerConfigurationInvalidatedAt: String
  invalidatingOptionSnapshotRevision: Int
  runtimeReceiptLinkSummary: RuntimeReceiptLinkSummary!
  promptDispatchSummary: ProviderPromptDispatchSummary!
  promptTurns: [ProviderPromptTurn!]!
}
extend type QueryRoot {
  providerExecutionTruthSchemaVersion: Int!
}
extend type GqlAgentExecution {
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  presentationRowId: ID
  providerExecutionTruth: ProviderExecutionTruth!
}
extend type RunStageTopologyOccurrence {
  presentationRowId: ID!
  compiledTaskId: ID!
  taskOccurrenceId: ID
  occurrenceSequence: Int
  occurrencePosition: TopologyOccurrencePosition!
  activeExecutionId: ID
  executionAttempts: [GqlAgentExecution!]!
  providerExecutionTruth: ProviderExecutionTruth!
  executionAssociationState: TopologyExecutionAssociationState!
  legacyAmbiguousExecutionCount: Int!
}
enum TopologyExecutionAssociationState {
  matched_v2 legacy_unique legacy_ambiguous not_started
}
enum TopologyOccurrencePosition {
  planned current previous
}
enum TopologyOccurrenceSourceKind {
  static_compiled owner_compiled dynamic_materialized legacy_flat
}
extend type RunStageTopologyNode {
  frozenWorkflowOrdinal: Int!
  legacyOrderUnverified: Boolean!
}
extend type RunStageTopologyOccurrence {
  sourceKind: TopologyOccurrenceSourceKind!
  sourceStableId: ID!
  frozenTaskOrdinal: Int!
  humanSourceOrdinal: Int!
}
extend type RunStageTopologyTransition {
  transitionId: ID!
  transitionOrdinal: Int!
}
extend type GqlMediationExecutionAttempt {
  providerExecutionTruth: ProviderExecutionTruth!
}
extend type GqlRuntimeEvent {
  agentExecutionId: ID
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  presentationRowId: ID
  timelineLaneId: ID!
  timelineLaneKind: TimelineLaneKind!
  timelineIdentityState: TimelineIdentityState!
}
extend type GqlTimelineRawDetailResult {
  timelineEventId: ID
  agentExecutionId: ID
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  presentationRowId: ID
  timelineLaneId: ID
  timelineLaneKind: TimelineLaneKind
  timelineIdentityState: TimelineIdentityState
}
```

Every Rust enum in this delta declares
`#[graphql(rename_items = "snake_case")]`; its GraphQL literal is exactly the
lowercase snake-case token shown above. The SDL snapshot and resolver fixtures
send every legal lowercase literal and reject uppercase, mixed-case, unknown,
and future values. No default async-graphql enum rename convention may silently
change this wire vocabulary.

`ProviderExecutionTruth` is the only new execution-level truth object on
GraphQL. `ProviderPromptConfigurationTruth` is its turn-owned child, not a
second execution projection. Agent execution, mediation attempt, and topology
occurrence all resolve the same Rust execution object and field set; no surface
re-declares or renames its members.
The generated alias map is exact: each lower-camel GraphQL member above maps to
the same-name snake-case JSON member, and each GraphQL enum maps to the closed
lowercase wire value. IDs, turn fields, summary containers, lists, and booleans
shown with `!` are non-null; historical rows may retain null execution/
configuration scalars. Topology still derives non-null presentation/compiled
IDs from frozen or migration identity even when no execution exists.

Every `ProviderExecutionTruth.schemaVersion` resolver returns the exact literal
`provider_execution_truth_v1`; every turn child returns
`provider_prompt_configuration_truth_v1`. Neither value is the integer probe
version. Snapshot and decoder fixtures reject null, another literal, or a
stringified integer. The execution-level accepted/configuration fields are the
owner's current receipt projection. Each turn child instead joins
`provider_generation_owner_bindings.prompt_turn_id` to the exact configuration
owner, attempt, generation, receipt, and invalidation evidence for that turn.
It never guesses from the owner's current pointer.

For a new turn, configuration owner kind/ID, attempt index, provider, requested
pair, and evidence state are present. A non-Codex turn uses
`not_applicable` with null configuration state and accepted fields. A migrated
turn that cannot be linked uses `legacy_unverified`; owner/attempt/generation
and configuration fields may then be null. `receipt_available` requires the
complete owner/attempt/generation tuple, configured state, accepted pair,
acceptance source, and verified time. `invalidated` additionally requires the
invalidation fields. `pending` and `receipt_unavailable` require the owner and
attempt but accepted fields are null.

Consequently a P079 repair shows original turn receipt/attempt A and repair turn
receipt/attempt B on the same parent physical generation A. The parent
execution's current receipt pointer may move to receipt B, but the original
turn continues to join receipt A and its terminal historical snapshot. A P086
target execution instead retains original physical generation A while the
continuation turn exposes continuation-owned attached generation B; admission
never updates the target pointer. Resolver fixtures assert both receipt pairs,
owners, attempts, physical generations, and sources simultaneously. A P079
option invalidation observed during repair invalidates future reuse of physical
generation A and the active repair receipt, but never rewrites the already
terminal original turn/receipt A; a P086 invalidation of B never rewrites A.

Aggregation reduces every authoritative turn and every runtime-receipt row, not
just the latest turn. `ProviderPromptDeliveryReducerV1` is complete and ordered:

1. any `dispatch_unknown` turn yields `unknown`;
2. otherwise any `legacy_unverified` receipt yields `legacy_unverified`;
3. otherwise select the greatest `(turn_index, prompt_turn_id)` specialized
   repair/continuation/Steward turn. `not_started|dispatch_pending` yields its
   `repair_pending|continuation_pending|steward_pending` value and `prompt_sent`
   yields the corresponding sent value;
4. only when no specialized turn exists, an original `dispatch_pending` or
   `prompt_sent` yields `original_pending` or `original_sent`; and
5. all authoritative turns `not_started`, or a planned occurrence with no
   execution, yields `not_started`. A historical execution with no turn is
   `legacy_unverified`, not zero-send.

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
topology occurrence with no execution is `not_started`, true, false; an
empty-turn historical execution is `legacy_unverified`, false, false.

Receipt linkage is never collapsed to an unexplained scalar. Each linked turn
exposes its own nullable `runtimeReceiptLinkState`; unlinked historical receipts
remain outside the turn array. `runtimeReceiptLinkSummary` counts all execution
receipts, including those unlinked rows, and computes `worstState` by the total
order `legacy_unverified > legacy_pre_prompt > linked_v2`; it is null only when
the execution has no runtime receipts. Counts are non-negative and sum to the
receipt row count. Thus original, both repair kinds, and every continuation can
have different link truth without losing evidence. Reducer fixtures enumerate
empty, homogeneous, and every mixed link-state multiset, including multiple
unlinked rows and sent-turn conflicts.

Execution association is explicit. A v2 occurrence match is `matched_v2`; the
single-task historical `agent_id` fallback is `legacy_unique`; more than one
candidate is `legacy_ambiguous`, leaves `activeExecutionId` and all runtime
identity fields null, and reports the bounded candidate count; no candidate is
`not_started`. The latest execution within a matched occurrence is selected by
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

Timeline identity is carried, never reconstructed from "latest agent". Both
`DomainEvent::RuntimeStatusChanged` and `RuntimeTimelineEvent` add nullable
`agent_execution_id`, `task_occurrence_id`, `task_occurrence_sequence`, and
`presentation_row_id`. Every new execution/provider event requires all four and
validates them against the same occurrence projection before publish; only
stage-level or pre-change legacy events may use null.

Every event also receives a non-null `TimelineLaneKeyV1`. An occurrence event
uses `timeline_lane_v1:<sha256>` over common-codec components
`[run_id, "occurrence", presentation_row_id]`, kind `OCCURRENCE`, and identity
state `MATCHED_OCCURRENCE_V2`. Every stage-level, run-level, or pre-change event
without a presentation row uses the single run lane derived from
`[run_id, "run_events"]`, kind `RUN_EVENTS`, and identity state
`UNASSOCIATED_RUN_EVENT`. It is never assigned to an occurrence by agent,
stage, provider, timestamp, or latest-execution lookup. The Timeline surface
always exposes a distinct `Run events` lane alongside the selected occurrence
lane; selecting an occurrence filters only its lane, while null-row events
remain reachable under `Run events`.

The existing `runtime_event_id` algorithm and `rte_<sha256>` spelling remain
byte-for-byte unchanged: its ordered inputs continue to be run, stage, agent,
event kind, nullable surface label, nullable session generation, timestamp, and
nullable detail digest with NUL separators. New execution, occurrence,
presentation, and lane fields are deliberately not hash inputs. Existing event
IDs, subscription cursors, bookmarks, and raw-detail handles therefore remain
valid. Historical and v2 golden vectors feed identical old inputs and require
the identical `rte_` ID; a separate tuple validator rejects an event whose new
identity fields do not join that event's durable execution/occurrence. The
GraphQL projection, subscription filters, and Swift
`P031RuntimeTimelineEventReadModel` preserve the tuple and lane fields end to
end.

`timeline_raw_details` is rebuilt with execution, occurrence,
occurrence-sequence, presentation, lane ID/kind, and timeline-identity state plus
a composite execution/occurrence integrity check. Persistence receives the
exact event tuple and inserts it directly; the current query that selects
`ORDER BY ae.started_at DESC LIMIT 1` by stage/agent/provider is deleted. The
raw-detail result echoes timeline event, execution, occurrence, occurrence
sequence, presentation, and lane identity, and the resolver verifies the
handle still points to that same tuple before returning bytes. Its existing six
status values and exact nullability are frozen:

| Status | Exact error reason | Raw bytes/digest | Identity fields |
|---|---|---|---|
| `available` | null | non-null and digest-verified | event and lane non-null; execution/occurrence tuple non-null for v2, nullable only for an authorized migrated unassociated row |
| `missing` | `handle_not_found` | all null | all identity fields null |
| `unauthorized` | `run_not_authorized` or `event_not_authorized` | all null | all identity fields null, even when a row exists |
| `stale` | `handle_expired` | all null | event and lane non-null after authorization; stored execution/occurrence tuple follows the same v2/legacy rule |
| `unavailable` | `storage_unavailable` | all null | event and lane non-null after authorization; stored execution/occurrence tuple follows the same v2/legacy rule |
| `digest_mismatch` | `digest_validation_failed` | all null | event and lane non-null after authorization; stored execution/occurrence tuple follows the same v2/legacy rule |

For every non-`available` result `rawDetail`, `rawDetailBytes`, and
`rawDetailDigest` are null and `errorReason` is non-null. `available` has null
`errorReason`. A row found with an impossible partial v2 tuple fails
`unavailable/storage_unavailable`; it is not downgraded to legacy. Swift groups,
expands, copies, and requests raw detail by `timelineLaneId`, event ID, and
handle. Interleaved chunks from two same-agent occurrences, retries inside one
occurrence, null-row stage events, terminal events, truncated raw details, old
`rte_` vectors, and deliberately swapped handles prove no event or payload
crosses lanes or rows.

### Bounded northbound and filesystem scope

The new accepted-truth readback is GraphQL-only in this slice. Domain owns
`ProviderExecutionTruthV1`; the GraphQL mapper above exposes it on the exact
run-detail execution, mediation-attempt, and topology-occurrence fields. The
complete checked-in `AppSchema::sdl()` snapshot, resolver tests, and shipped
Swift query/decoder tests are the wire authority. There is no second JSON
serializer or report/resource projection to keep in parity.

Existing MCP negotiation remains `2024-11-05`. Existing `run://`,
`report://`, `reports.get`, `tools/list`, and `tools/call` envelopes,
schemas, redaction, and authorization remain byte-compatible; this proposal
does not advertise `outputSchema` or return `structuredContent`. Their
existing `model` values remain requested/planned compatibility values and are
not relabeled as accepted truth. A later MCP protocol or report-schema uplift
requires its own versioned proposal and old/new compatibility matrix.

Generated run reports and artifact bytes are also unchanged. This proposal does
not add a report candidate, materializer, truth epoch, artifact lease, or
canonical-report rewrite. It does not alter provider filesystem permissions,
Codex `danger-full-access`, worktree write grants, Steward output paths, or
adapter launch containment. Accepted model/effort truth is durable database
state read through GraphQL; it is not injected into provider-authored files.
The structural gate rejects any new `run-v2://`, report variant,
`ProviderFilesystemProfileV1`, `run_report.materialize`, or MCP
`structuredContent` implementation in this slice.

Steward still uses the same internal configuration receipt and prompt authority,
but this proposal adds no public Steward GraphQL/MCP/resource lane DTO.
The existing MCP tools remain exactly `steward.list_analyses` and
`steward.get_analysis`; their names, input/output schemas, principal filtering,
and redaction are byte-compatible, and no `steward.list`/`steward.get` aliases
are introduced. Steward verification in this slice is through repository state
and provider-free engine tests.
A future operator-facing Steward identity surface must define its own exact
authorization and cross-surface contract rather than piggybacking on run-detail
types.

The shipped `P031GraphQLDocumentSet.runDetail` property with operation
`P031RunDetail`, not a test-only query, adds the exact selections below.
`P031ProviderExecutionTruthFields` is one source literal interpolated at each
marked selection so fields cannot drift:

```graphql
fragment P031ProviderExecutionTruthFields on ProviderExecutionTruth {
  schemaVersion agentExecutionId taskOccurrenceId taskOccurrenceSequence
  executionProvider requestedModel requestedEffort acceptedModel acceptedEffort
  providerConfigurationState configurationEvidenceState acceptanceSource
  providerConfigurationVerifiedAt providerConfigurationInvalidatedAt
  invalidatingOptionSnapshotRevision
  runtimeReceiptLinkSummary {
    linkedV2Count legacyPrePromptCount legacyUnverifiedCount worstState
  }
  promptDispatchSummary {
    originalTurnState latestTurnKind latestTurnIndex latestTurnState
    deliveryTruth noPromptSent hasUnresolvedUnknown
  }
  promptTurns {
    promptTurnId promptKind turnIndex promptOwnerKind promptOwnerId dispatchState
    dispatchStartedAt promptSentAt dispatchUnknownAt failureCode
    runtimeReceiptLinkState
    configurationTruth {
      schemaVersion configurationOwnerKind configurationOwnerId
      configurationAttemptIndex agentExecutionId taskOccurrenceId continuationId
      sessionGenerationId provider requestedModel requestedEffort
      acceptedModel acceptedEffort providerConfigurationState
      configurationEvidenceState acceptanceSource
      providerConfigurationVerifiedAt providerConfigurationInvalidatedAt
      invalidatingOptionSnapshotRevision
    }
  }
}

# Added under stages.executions
id taskOccurrenceId taskOccurrenceSequence presentationRowId
providerExecutionTruth { ...P031ProviderExecutionTruthFields }

# Added under each runStageTopology node
frozenWorkflowOrdinal legacyOrderUnverified

# Exact runStageTopology.occurrences selection
presentationRowId compiledTaskId taskOccurrenceId occurrenceSequence
occurrencePosition sourceKind sourceStableId frozenTaskOrdinal
humanSourceOrdinal
activeExecutionId executionAssociationState legacyAmbiguousExecutionCount
agentId agentTitle taskName status provider model effort executionCount
providerExecutionTruth { ...P031ProviderExecutionTruthFields }
executionAttempts {
  id status startedAt completedAt
  providerExecutionTruth { ...P031ProviderExecutionTruthFields }
}

# Exact runStageTopology.transitions selection
transitionId transitionOrdinal toStageId toLabel detail

# Added under activeAgentExecutions
taskOccurrenceId taskOccurrenceSequence presentationRowId
providerExecutionTruth { ...P031ProviderExecutionTruthFields }
```

`RunStageTopologyOccurrence` therefore adds non-null ordered
`executionAttempts: [GqlAgentExecution!]!`, scoped only by exact occurrence ID
and ordered `started_at DESC, id DESC`. `P031RuntimeTimelineEventReadModel` and
the shipped `P031GraphQLDocumentSet.runtimeStatusChanged` property with
operation `P031RuntimeStatusChanged` select/decode `agentExecutionId`,
`taskOccurrenceId`, `taskOccurrenceSequence`, `presentationRowId`,
`timelineLaneId`, `timelineLaneKind`, and `timelineIdentityState`. The
shipped `P031GraphQLDocumentSet.timelineRawDetail` property with operation
`P031TimelineRawDetail` and `P031TimelineRawDetailReadModel` select/decode the
same identity/lane fields plus `timelineEventId`. Its document also selects the
existing status/raw/error fields so the nullability matrix above is decoded in
one response. No production document reconstructs identity by agent ID.

The exact shipped DTO changes are:

- `P031StageAgentExecutionReadModel` adds occurrence ID/sequence,
  presentation-row ID, and non-null `P031ProviderExecutionTruthReadModel`;
- `P031ProviderPromptTurnReadModel` adds non-null
  `P031ProviderPromptConfigurationTruthReadModel`; its presence-aware decoder
  implements the exact new/non-Codex/legacy nullability matrix rather than
  reading the execution-level current receipt;
- `P031ActiveAgentExecutionReadModel` adds occurrence ID/sequence,
  presentation-row ID, and non-null truth;
- `P031RunStageTopologyOccurrenceReadModel` adds every exact topology field,
  including non-null `humanSourceOrdinal`, and
  `[P031OccurrenceExecutionAttemptReadModel]` rather than only aggregate count;
- `P031RunStageTopologyReadModel` adds frozen workflow ordinal and legacy-order
  state, while `P031RunStageTopologyTransitionReadModel` adds transition ID and
  ordinal;
- `P031RuntimeTimelineEventReadModel` and
  `P031TimelineRawDetailReadModel` add the exact event identity and lane tuple;
  the latter preserves all six existing status cases and their closed
  nullability; and
- `P031RunDetailReadModel` retains its current arrays and decodes the enriched
  objects in place; there is no parallel test-only read model.

Occurrence position is owned only by the topology occurrence projection; it is
not copied onto execution/event rows where it could become stale. After decode,
`P031RunDetailReadModel` constructs mandatory
`OccurrencePresentationJoinV1(runID, presentationRowID, taskOccurrenceID,
stageID, occurrencePosition, humanSourceOrdinal)` entries from topology rows.
Every v2 execution, matched-occurrence timeline event, raw-detail result, and
active-agent row joins by exact `(runID, presentationRowID)` and, when present,
must match the same task-occurrence ID. Missing, duplicate, cross-run, or
position/source-mismatched joins are typed schema failures; an explicit
`unassociated_run_event` is the only event shape that bypasses the join and it
must target the run-events lane. Position changes are published by replacing
this join snapshot, never by rewriting historical events.

Every DTO declares every `CodingKey` and uses a custom decoder that distinguishes
`container.contains(key) == false` (typed schema mismatch) from explicit null
(valid state). Checked-in GraphQL and Swift fixtures cover historical Codex,
non-Codex, pre-session configuration failure, mediation, both P079 repairs, all
three P086 continuation modes, empty legacy turns, same-agent occurrences, and
schema mismatch. A document snapshot test byte-compares the complete production
`P031RunDetail`, `P031RuntimeStatusChanged`, and `P031TimelineRawDetail`
operation strings from those exact three properties; a decoder test fails if
any required selected field is absent.

### Closed public values and nullability

Rust enums, GraphQL enums, and Swift enums are generated or byte-compared from
these closed wire domains:

| Domain | Exact JSON values |
|---|---|
| configuration state | `configuring`, `configured`, `invalidated_after_acceptance`, `failed_before_prompt`, `cancelled_before_prompt`, `legacy_unverified` |
| configuration evidence | `pending`, `receipt_available`, `invalidated`, `receipt_unavailable`, `not_applicable`, `legacy_unverified` |
| effective provider capability | `codex_exact_pair_v1`, `not_applicable_v1`, `legacy_best_effort_v0` |
| acceptance source | `fresh_negotiation`, `reused_session_generation`, `attached_session_reverification` |
| prompt kind | `original`, `code_writer_completion_repair`, `output_contract_repair`, `work_continuation_live_handle`, `work_continuation_resurrection`, `work_continuation_output_only`, `steward_analysis` |
| prompt owner kind | `invoke_agent`, `p017_mediation`, `p058_escalation`, `p079_repair`, `p079_fallback_child`, `p086_continuation`, `steward_agent_lane` |
| configuration owner kind | `agent_execution`, `p086_continuation`, `steward_agent_lane` |
| prompt dispatch state | `not_started`, `dispatch_pending`, `prompt_sent`, `dispatch_unknown` |
| delivery truth | `not_started`, `original_pending`, `original_sent`, `repair_pending`, `repair_sent`, `continuation_pending`, `continuation_sent`, `steward_pending`, `steward_sent`, `unknown`, `legacy_unverified` |
| runtime-receipt link | `linked_v2`, `legacy_pre_prompt`, `legacy_unverified` |
| topology association | `matched_v2`, `legacy_unique`, `legacy_ambiguous`, `not_started` |
| topology occurrence source | `static_compiled`, `owner_compiled`, `dynamic_materialized`, `legacy_flat` |
| topology occurrence position | `planned`, `current`, `previous` |
| timeline lane kind | `occurrence`, `run_events` |
| timeline identity state | `matched_occurrence_v2`, `unassociated_run_event` |
| timeline raw-detail status | `available`, `missing`, `stale`, `unauthorized`, `unavailable`, `digest_mismatch` |
| timeline raw-detail error | `handle_not_found`, `handle_expired`, `run_not_authorized`, `event_not_authorized`, `storage_unavailable`, `digest_validation_failed` |

Unknown enum strings are schema errors, not display fallbacks. Nullability is
also normative:

- a new `codex_exact_pair_v1` execution always has requested model and effort;
  historical and non-Codex requested fields retain their existing optionality;
- `configured` requires accepted model/effort, wire pair, acceptance source,
  generation/session binding, receipt digest, verified time, and
  `receipt_available` together;
- `invalidated_after_acceptance` preserves the historical accepted pair and
  receipt but requires evidence `invalidated`, the invalidating option-snapshot
  revision/digest, non-null invalidated time/revision readback, and UI copy that
  runtime identity is unknown; it is never rendered as actual;
- every non-invalidated state requires invalidated time/revision readback null;
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
  scalars are null plus a non-null `not_started` summary, with configuration
  evidence `pending` for planned exact Codex, `not_applicable` for planned
  non-Codex, or `legacy_unverified` for a legacy row;
- every prompt turn has a non-null configuration-truth shell. New exact-pair
  turns require owner kind/ID and attempt index; `receipt_available` and
  `invalidated` additionally require generation and the complete accepted pair;
  non-Codex uses `not_applicable`, while only migrated unlinked turns may use
  `legacy_unverified` with nullable owner/attempt/generation fields;
- every prompt turn's receipt-link state is null when no receipt links to that
  turn; the non-null link summary counts linked and unlinked receipts, sums to
  total receipt count, and uses the frozen worst-state order;
- receipt JSON always emits every declared nullable key as explicit null;
  GraphQL omits none of the declared fields, and Swift treats omission as a
  schema mismatch; and
- every runtime event has non-null lane ID/kind/identity state. Raw-detail
  identity nullability is determined only by the exact six-status matrix above,
  never by decoder convenience.

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
| Cancelled during configuration | Accepted pair absent; configuration is terminal | `Cancelled before configuration accepted: Requested Codex - GPT-5.6 Terra - High - No prompt sent - Acceptance unverified` |
| Configured / not started | Response-verified pair; prompt not attempted | `Configured: Codex - GPT-5.6 Terra - High - Prompt not started` |
| Configuration invalidated before prompt | Historical accepted receipt exists but live option snapshot changed; prompt not attempted | `Configuration invalidated: Requested Codex - GPT-5.6 Terra - High - No prompt sent` |
| Start failed after configuration | Response-verified pair; execution failed while turn stayed `not_started` | `Start failed: Codex - GPT-5.6 Terra - High - Configuration accepted - No prompt sent` |
| Cancelled before prompt | Response-verified pair; dispatch remains `not_started` | `Cancelled before prompt: Codex - GPT-5.6 Terra - High - Configuration accepted - No prompt sent` |
| Dispatch pending | Response-verified pair; delivery not yet known | `Starting: Codex - GPT-5.6 Terra - High` |
| Prompt sent / running | Response-verified pair and durable prompt sent | `Using: Codex - GPT-5.6 Terra - High` |
| Prompt sent / completed | Response-verified pair and durable prompt sent | `Used: Codex - GPT-5.6 Terra - High` |
| Prompt sent / failed | Response-verified pair; execution failed later | `Failed after prompt: Codex - GPT-5.6 Terra - High` |
| Prompt sent / cancelled | Response-verified pair; execution cancelled later | `Cancelled: Codex - GPT-5.6 Terra - High` |
| Dispatch unknown | Response-verified pair; delivery ambiguous | `Prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Configuration invalidated during prompt | Provider option update makes runtime identity and delivery unsafe | `Runtime identity unknown: Requested Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Repair pending | Original prompt sent; repair turn pending | `Using: Codex - GPT-5.6 Terra - High - Repair starting` |
| Repair sent | Original and repair prompts durably sent | `Using: Codex - GPT-5.6 Terra - High - Repair prompt sent` |
| Repair unknown | Original sent; repair delivery ambiguous | `Repair prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Continuation pending | Original sent; P086 turn pending | `Using: Codex - GPT-5.6 Terra - High - Continuation starting` |
| Continuation sent | P086 turn durably sent | `Using: Codex - GPT-5.6 Terra - High - Continuation prompt sent` |
| Continuation unknown | P086 delivery ambiguous | `Continuation prompt delivery unknown: Codex - GPT-5.6 Terra - High - Do not retry automatically` |
| Configuration failure | Requested pair present; accepted pair absent | `Configuration failed: Requested Codex - GPT-5.6 Terra - High - No prompt sent - Acceptance unverified` |
| Legacy generic | Frozen requested identity; provider acceptance unavailable | Exact legacy table below; every row ends in `Unverified` |
| Retry/fallback | Latest execution for the same task occurrence | Codex uses that execution's accepted pair and dispatch state |

If `configured` lacks either accepted field, the Codex readback is internally
inconsistent. It renders `Runtime identity unavailable`, exposes diagnostic
Help text, and must be caught by the gate. It must not fall back to planned
truth.

`legacy_ambiguous` is a legal formatter input, not a schema failure. It renders
`Runtime identity unavailable - Multiple legacy executions`, Help text with the
bounded candidate count, and no accepted/requested runtime pair. Planned task
identity may remain on its separate planned line. It never selects one legacy
execution. A configured execution that fails before permit is likewise the
legal `Start failed after configuration` row above rather than an impossible
tuple.

Legacy generic Codex status is also closed rather than assembled from an
unspecified prefix:

| Legacy state | Exact operator copy |
|---|---|
| planned | `Planned: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| starting | `Starting: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| running after prompt | `Running: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| completed after prompt | `Completed: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| failed after prompt | `Failed: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| cancelled after prompt | `Cancelled: Codex - GPT-5.6 (variant unspecified) - High - Unverified` |
| delivery unknown | `Prompt delivery unknown: Codex - GPT-5.6 (variant unspecified) - High - Do not retry automatically - Unverified` |

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
| Repair pending | `Running: Claude - opus - High - Repair starting - Acceptance unverified` |
| Repair sent | `Running: Claude - opus - High - Repair prompt sent - Acceptance unverified` |
| Repair unknown | `Repair prompt delivery unknown: Claude - opus - High - Do not retry automatically` |
| Continuation pending | `Running: Claude - opus - High - Continuation starting - Acceptance unverified` |
| Continuation sent | `Running: Claude - opus - High - Continuation prompt sent - Acceptance unverified` |
| Continuation unknown | `Continuation prompt delivery unknown: Claude - opus - High - Do not retry automatically` |
| Historical pending | `Planned: Claude - opus - High - Delivery unverified` |
| Historical running | `Running: Claude - opus - High - Delivery unverified` |
| Historical completed | `Completed: Claude - opus - High - Delivery unverified` |
| Historical failed | `Failed: Claude - opus - High - Delivery unverified` |
| Historical cancelled | `Cancelled: Claude - opus - High - Delivery unverified` |

A Codex-to-non-Codex fallback uses the provider-neutral row and never inherits
the prior Codex accepted pair. A non-Codex-to-Codex fallback must complete the
exact Codex transaction. Missing model or effort segments are omitted, not
invented.

Cancellation while still `not_started` is provably unprompted. Cancellation
after `dispatch_pending` but before durable `prompt_sent` settles
`dispatch_unknown`; cancellation must not erase delivery ambiguity.
For Codex, the formatter selects the `Cancelled during configuration` row only
when accepted model/effort and verified time are all null. It selects
`Cancelled before prompt` only when the complete accepted pair and receipt are
present. Those two strings, Help values, and accessibility values are required
to differ byte-for-byte. A partial accepted pair is corruption and renders
`Runtime identity unavailable`; it never selects either cancellation row.

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

The info button uses the existing button action, and the presentation reducer
defines `openIdentity`, `copyIdentity`, `closeIdentity`, and
`selectedRowRemoved` commands. Opening records the triggering
`presentationRowId`; closing requests focus restoration to that row. If it
disappears while open, the reducer dismisses the popover, clears stale copy
state, and chooses the same deterministic surviving-row fallback used by Run
Inspector (or the stage heading when none remains). Pure tests invoke every
command and hosted-view tests assert the resulting focus target and accessibility
values. This proposal does not claim that a local hosted test proves physical
Space/Return/Escape or VoiceOver event delivery; that remains in the existing
remote UI gate.

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
`OccurrenceDiscriminatorV1` is
`Planned task <human_source_ordinal + 1> in <stage label>` for a planned row,
`Occurrence <occurrence_sequence + 1> for task <human_source_ordinal + 1> in <stage label>`
for a durable sequenced row, or
`Legacy task <human_source_ordinal + 1> in <stage label>` for a legacy row
without sequence. Ordinals use canonical ASCII base-10 with no grouping,
locale digits, sign, decimal separator, or `NumberFormatter`; Swift formats the
checked non-negative integer with the frozen POSIX-independent integer codec.
The migration never
emits more than one unsequenced legacy topology row for one source; multiple
unmatched historical executions are represented as that row's bounded
`legacyAmbiguousExecutionCount`, not duplicate spoken rows. Repeated task names
and separately materialized dynamic tasks are distinguished by unique persisted
human source ordinal; repeated occurrences of one task are distinguished by
durable occurrence sequence. No digest, UUID, provider session
ID, or abbreviated opaque identifier appears in a spoken label. The
row label is exactly
`<task name>. <occurrenceDiscriminator>. <fullIdentity>. Status: <status>. Attempts: <count>.`
Here `<status>` is not free-form: `AccessibilityExecutionStatusV1` maps the
legal formatter state exactly to `Planned`, `Configuring`, `Configured`,
`Starting`, `Running`, `Completed`, `Failed`, `Cancelled`,
`Prompt delivery unknown`, `Runtime identity unknown`, or
`Runtime identity unavailable`. `<count>` uses the same non-negative canonical
ASCII integer codec as the ordinals. Unknown state, negative count, or a status
not selected by the legal-state table is `identity_contract_invalid`; there is
no locale-aware or description-based fallback.
The info control label is
`Show full runtime identity for <task name>, <occurrenceDiscriminator>`, the copy
control is `Copy full runtime identity for <task name>, <occurrenceDiscriminator>`,
and the close control is `Close runtime identity details`. Unknown delivery adds
the exact hint `Automatic retry is blocked.` to the row value; legacy ambiguity
adds `Multiple legacy executions; runtime identity is unavailable.` Tests assert
labels, values, hints, child order, and reducer actions byte-for-byte. Same-agent
and repeated-task fixtures assert distinct labels and action targets. Machine
uniqueness is separate: every accessibility identifier is the canonical
`PresentationTargetV1` identifier below and may contain the full hashed row ID;
it is never used as accessibility label, value, hint, Help text, or spoken
custom-action name.
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
carries the same `presentationRowId`, nullable `taskOccurrenceId`, and nullable
`occurrenceSequence`; only topology occurrence rows carry the authoritative
`planned|current|previous` position, and all other surfaces consume the exact
`OccurrencePresentationJoinV1` above. Semantic
selection keys by presentation row ID, never by agent ID. Timeline derives the
selected occurrence's `timelineLaneId`; its separate `Run events` selection
uses the run-events lane and no synthetic presentation row. Two same-agent
occurrences therefore remain distinct rows with independent status, attempts,
model, effort, and event lanes. Visual, Help, popover, copy, and accessibility
strings are generated from the same formatter result.

Interactive targets are surface-qualified and never identify a non-row element
with a nullable row ID. `PresentationSubjectV1` is the required tagged union:

| Tag | Exact payload |
|---|---|
| `occurrence` | `presentation_row_id` |
| `timeline_event` | `timeline_lane_id`, `timeline_event_id` |
| `stage_heading` | `stage_id` |
| `run_events` | `timeline_lane_id` |
| `empty_summary` | no payload |

`PresentationTargetV1` is the exact tuple
`(run_id, surface, subject, control_kind)`, where surface is closed to
`overview | stages | timeline | run_inspector` and control kind is closed to
`row`, `info`, `copy`, `popover`, `close`, `event`, `stage_heading`,
`run_events`, or `empty_summary`. Subject/control compatibility is closed:
row/info/copy/popover
use `occurrence`, event uses `timeline_event`, stage heading uses
`stage_heading`, run-events uses `run_events`, empty summary uses
`empty_summary`, and close preserves the subject of the popover it closes. Its
machine identifier uses the common codec with domain
`chainworks.presentation_target.v1` plus the union tag and exact payload; two
timeline events in one lane, two stage headings, or a heading and empty summary
can never compare equal. A row ID remains the cross-surface semantic selection
key, but it is never by itself a focus, popover, anchor,
accessibility-action, event, heading, or copy target.

`SelectedRowSnapshotV1` stores the last selected row's presentation-row ID,
source stable ID, normalized index, occurrence position, and stage ID. The
single `RunOccurrenceSelectionStateV1` contains nullable `selectedRunID`,
`selectedPresentationRowID`, nullable `selectedRowSnapshot`, non-null
`activeSurface`, nullable `popoverTarget: PresentationTargetV1`, and nullable
`focusTarget: PresentationTargetV1`. There is no per-run dictionary and no
view-local `selectedAgentID`.

`P031RunsHomeViewModel` owns one `RunsWorkbenchPresentationModel`. Overview,
Stages, Timeline, Run Inspector, popover, and focus events all call one pure
`RunOccurrenceSelectionReducer.reduce(state:event:rows:)`; each user event
carries its complete `PresentationTargetV1`, and views receive only reducer
bindings/actions. `runChanged` clears selection, prior snapshot, popover, and
focus before choosing the new run's default. `rowsChanged` uses the retained
snapshot, not a lookup in the already-replaced row array: it first maps a
removed planned row to the current row with the same source stable ID;
otherwise it chooses the row now occupying the prior normalized index, then the
last preceding row, then that stage's heading, then the empty-run summary. It
rebuilds focus for `activeSurface` and closes a popover whose exact target no
longer exists; it never transfers a `stages` anchor to `overview` merely because
the row ID matches. Selection and every successful rows change refresh the
snapshot from the chosen row before returning.

The view model also owns one `RunDetailPublicationOwner`. Beginning a run load
atomically increments a monotonic `load_generation`, cancels the prior request
and subscriptions, clears prior run-detail rows, and emits `runChanged` before
starting the new request. Every initial GraphQL response, subscription update,
raw-detail response, and topology retry callback carries its captured
`(run_id, load_generation)`. Publication is accepted only when both values
equal the owner's current tuple; a stale callback is dropped without mutating
rows, selected snapshot, popover, focus, timeline lane, or error state. A
successful replacement subscription is installed only under the same CAS and
old-generation cancellation cannot clear the new subscription. Pure and hosted
fixtures delay run A, select run B, publish B, then deliver A responses and
updates in every order; B remains byte-identical and no A target can reappear.

Run Inspector deletes `activeTimelineAgents.first`; it resolves the
selected `RunStageTopologyOccurrenceV2`, then shows that occurrence's complete
attempt list ordered by `started_at DESC, agent_execution_id DESC` and uses its
latest attempt only for the summary identity. With no explicit selection it
chooses the first `current` row in normalized presentation order, then the first
`previous` row, then the first planned row. Popover and focus consume that exact
reducer result rather than implementing a second fallback rule. An agent ID is
never a selection key. Pure reducer tests cover all surface/control pairs,
same-row targets on two surfaces, removed-row snapshot fallback, planned to
current replacement, popover invalidation, run switching, and run-events lane.
Hosted tests cover two simultaneous same-agent occurrences, explicit attempt
inspection, retry in one occurrence, loop replacement, row removal, and
deterministic fallback without model/effort crossover.

Focus proof uses the production focus bridge, not reducer state alone. The
hosted suite mounts the real row, heading, timeline-event, and popover controls
in an `NSHostingView` inside a key `NSWindow`, with the production
`.focused($focusTarget, equals: target)` modifiers and
`PresentationTargetV1` accessibility identifiers. It dispatches each reducer
focus action on the main actor, drains the run loop, and asserts both the bound
`FocusState` value and the AppKit first-responder/accessibility identifier of the
actual control. Row removal, stage-heading fallback, popover close restoration,
timeline-event selection, and stale run-A publication after run B each fail if
the modifier is absent or attached to another target.

## Failure Behavior

| Failure | Typed result | Prompt dispatch |
|---|---|---:|
| Fresh generic or unapproved catalog pair | compile failure | 0 |
| Model option/value unavailable | `ACP_CODEX_MODEL_UNAVAILABLE` | 0 |
| Model response lacks matching current value | `ACP_CODEX_MODEL_NOT_ACCEPTED` | 0 |
| Updated effort option/value unavailable | `ACP_CODEX_EFFORT_UNAVAILABLE` | 0 |
| Final response does not verify both values | `ACP_CODEX_EFFORT_NOT_ACCEPTED` | 0 |
| Changed/malformed option update before prompt | `ACP_PROVIDER_CONFIGURATION_INVALIDATED` | 0 |
| Changed/malformed option update after write starts | configuration/delivery unknown; generation invalidated | unknown |
| Accepted-truth persistence fails | `ACP_PROVIDER_CONFIGURATION_PERSISTENCE_FAILED` | 0 |
| Acceptance/receipt malformed or digest mismatch | `ACP_PROVIDER_CONFIGURATION_EVIDENCE_INVALID` | 0 |
| Owner receipt missing or owner-kind fields inconsistent | `ACP_PROVIDER_CONFIGURATION_OWNER_INVALID` | 0 |
| Provider process identity absent/ambiguous | owner quarantine; `ACP_PROVIDER_PROCESS_IDENTITY_UNVERIFIED` | 0 or unknown per turn state |
| Cancellation wins during configuration | `cancelled_before_prompt` | 0 |
| Original-owner reused generation evidence mismatch | close generation and negotiate fresh once | 0 on old session |
| P079 repair/fallback or P086 generation evidence mismatch | fail typed owner; transparent fresh fallback forbidden | 0 |
| P079 fallback provenance/operation/lease join mismatch | `ACP_PROMPT_OWNER_INVALID`; settle fallback attempt | 0 |
| P086 continuation lacks execution/occurrence/turn/work-item binding | `ACP_PROMPT_OWNER_INVALID` | 0 |
| P086 resume context missing/digest or target binding mismatch | admission rejected before launch | 0 |
| P086 resume capability absent | `ACP_P086_RESUME_UNSUPPORTED` | 0 |
| P086 pre-response option update or response/catalog/correlation invalid | `ACP_P086_RESUME_CONFIGURATION_UNAVAILABLE`; identity-safe reap | 0 |
| P086 atomic admission insert fails | complete transaction rollback; no accepted response | 0 |
| Steward invocation lacks analysis/lane/agent/work-item binding | `ACP_PROMPT_OWNER_INVALID` | 0 |
| Dispatch permit loses to cancellation/ownership/epoch CAS | `ACP_PROMPT_DISPATCH_PREPARE_FAILED` | 0 |
| Initial prompt-turn CAS returns `Missing` | `ACP_PROMPT_TURN_MISSING`; owner blocked/failed | 0 |
| Bounded write deadline or cancellation wins after permit | `ACP_PROMPT_DISPATCH_UNKNOWN`; manager interrupts and settles provider turn | unknown |
| Transport send/flush fails after dispatch pending | `ACP_PROMPT_DISPATCH_UNKNOWN` | unknown |
| Prompt-sent persistence fails after transport success | `ACP_PROMPT_DISPATCH_UNKNOWN` | sent or unknown |
| Final sent CAS and unknown/quarantine settlement both fail after possible I/O | `PromptAuthorityUnsettledAfterIo`; daemon failed-serve | unknown |
| DbWriter uncertain-after-start cannot reconcile | `OperationObservationV1::Unknown`; admission remains closed for that owner | 0 or unknown according to whether transport had started |
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

`codex-model-truth` is also the composition signoff for every owner boundary
touched by this proposal. It invokes the shared test functions and frozen test
arrays owned by `proposal-027`, `proposal-058`, `proposal-075`, `proposal-079`,
and `proposal-086`; it does not recursively invoke any shell alias and therefore
does not reacquire their gate locks. All five standalone aliases remain
supported and must also pass. The composed gate and all five aliases record and
require the same committed `HEAD`. Before the first leg, between legs, and after the final leg, the gate
verifies that `HEAD` is unchanged and that the generated proposal-owned path
manifest has no tracked or untracked changes. A proof assembled from different
commits, or from an implementation file absent from the manifest, is invalid.

| Owner | Required proof in `codex-model-truth` |
|---|---|
| `workflow` + `domain` | Exact seven-profile matrix; per-profile frozen capabilities and effective fallback contract; validated Steward catalog; fresh generic/invalid rejection; legacy replay; sealed provenance union including typed P079 fallback; exact ten-ID manifest; typed P058 owner; `OutputContractRepair` work-item kind; complete compiled-coordinate, condition, binding, dynamic-key, deterministic enqueue-time occurrence allocation, unique human-source ordinal, legacy ordering, sequence, and presentation vectors |
| `acp` fake provider + `engine` dispatch | Response-closed negotiation plus generation-owned `config_option_update` snapshot; durable secret-safe P086 resume context, post-launch capability proof, exact `session/resume`, pre-response-update rejection, response catalog, ordered post-response updates, and identity-safe zero-send reap matrix; separate new/existing-generation reservations; many-to-one ownership with one prompt-through-terminal manager; acyclic typed authority/control ports and permit-only API; bounded broker/config/send/terminal-settlement/cleanup; complete P017/P058/P079/P086/Steward reducers and collateral matrix; owner-scoped versus generation cancellation; launch barrier/process identity; bidirectional fallback; Claude aliases unchanged |
| `db` + `engine` recovery | Lower-layer lock-guarded staged/tracked-equal preflight; complete registered Class A operation/result-codec/natural-row set with uncancellable late-commit supervisor; separate acknowledgement certainty and operation-specific domain outcomes; exact generation-binding receipt/failure matrix; active owner attempts/receipts/invalidations/failures; exact Steward turn-0/one-retry turn-1 allocation and three-phase cancellation reducer; prompt authority/quarantine; immutable migration-095 checksum plus complete migration-100 P079 DDL, strict canonical/quarantine accounting, terminal guards, and atomic validation settlement; immutable P058 execution/ledger/tier authority; deterministic occurrence enqueue/copy-validation, unique human ordinal, and legacy backfill/seed/restart; exhaustive old P086 classifier; sealed legacy envelopes; closure-owned replay authorization; paused-before-commit mutation-fence fixtures for every writer |
| `daemon` composition | One `open_runtime_database` guard and no lock reacquisition; ready/failed bootstrap owner union with failed owner retaining `PreflightLockGuard`; production construction of upgrade coordinator, durable authority, ACP manager, invocation/invalidation coordinators, process-control port, `DbWriter`, and the sole `FirstFatalCoordinator`; one prebound tri-state `starting|normal|failed` router; durable first-fatal persist-before-notify; exact Operator-only `DaemonStatus` AST whitelist and zero-DB minimal routes |
| `graphql-server` | Byte-equal complete `AppSchema::sdl()` with explicit lowercase snake-case enum literals, uppercase/unknown negatives, probe matrix, and exact schema-version literals; one non-null execution-level truth object plus turn-owned configuration truth; complete latest-specialized-turn reducer; simultaneous P079 receipt A/B on physical generation A and P086 physical generation A/B readback; exact occurrence attempt list and mandatory occurrence-presentation join; old `rte_` compatibility vectors; non-null occurrence/run-events lanes; all six raw-detail status/nullability rows; mediation/topology mapping; structural proof that this slice does not change MCP/report/resource schemas |
| Swift focused and hosted-view tests | Complete production `P031GraphQLDocumentSet.runDetail`/`runtimeStatusChanged`/`timelineRawDetail` snapshots for operations `P031RunDetail`/`P031RuntimeStatusChanged`/`P031TimelineRawDetail` and presence-aware DTO decoding; lockstep restart; complete state/ambiguity/start-failure/invalidation matrices with byte-distinct pre/post-acceptance cancellation; one exact formatter plus independent stdlib oracle across shipped surfaces and host locales; mandatory occurrence-presentation join; generation-qualified run publication; tagged injective targets; unique human occurrence accessibility with opaque IDs only in machine identifiers; exact topology rules; real `NSHostingView`/`.focused` first-responder proof, without claiming remote keyboard/VoiceOver event delivery |

Swift proof is two independent invocations and result bundles:

1. `codex-model-truth-pure.xcresult` runs
   `ProviderExecutionIdentityFormatterTests`,
   `ProviderExecutionTruthDecodingTests`,
   `ProviderPromptConfigurationTruthDecodingTests`,
   `P031RunDetailContractTests`,
   `P031TimelineIdentityContractTests`,
   `RunOccurrenceSelectionReducerTests`, and
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

1. model success changes the effort option set and the adapter resolves effort
   only from the updated response snapshot;
2. model or effort JSON-RPC success with mismatched final `currentValue`,
   missing/malformed options, ambiguity, unknown value, or incompatible effort;
3. `config_option_update` before, between, and after both set-option responses
   updates the generation snapshot in wire-observation order;
4. changed or malformed option updates after receipt commit but before prompt,
   during write/response, and after terminal settlement produce the exact
   zero-send, unknown, or next-owner invalidation result and never reuse stale
   acceptance;
5. accepted-truth receipt persistence, invalidation persistence, and minimal
   failure settlement faults prove the exact prompt count and synchronous fatal
   admission-fence behavior;
6. a legacy generic request proves the old best-effort path remains reachable,
   while Claude alias matching remains byte-compatible;
7. `reserve_new_generation` and `reserve_existing_generation` races prove one
    globally unique prompt-turn binding, idempotent same-generation replay,
    conflicting cross-generation reuse, monotonic owner receipt-attempt
    allocation with no replay/loser gaps, zero loser I/O, and every legal/illegal
    `admitted` through terminal receipt-versus-failure reference tuple;
8. matching existing-generation evidence derives an owner receipt without
    changing the physical generation allocator, while consuming exactly one
    owner receipt-attempt; missing/mismatched evidence closes the old handle and
    permits only the owner-kind policy's explicit next action;
9. P086 admission binds the immutable secret-safe resume-context ID/digest;
   attachment launches/binds identity before proving advertised resume
   capability, sends exact stored cwd/roots/MCP inputs, rejects every
   pre-response authority update without reordering, seeds only from the
   correlated non-empty response catalog, orders later updates, reverifies the
   pair, identity-safely reaps every post-launch failure, and never sends
   `session/new`, `session/load`, or a prompt for any negative permutation;
10. original success followed by both P079 repair kinds proves independent turns,
    distinct receipt attempts on the same parent physical generation, immutable
    original terminal-turn truth under repair-time invalidation, the closed
    prompt-to-configuration-owner mapping, typed
    `OutputContractRepair` work items, one logical budget, bounded zero-send
    attempt leases, TTL settlement, and atomic child creation;
11. `p079.provider_fallback_child` carries the typed provenance, owner kind,
    operation/attempt/lease authority, source occurrence, target binding digest,
    and initial one-generation permit; collateral loss allows zero transparent
    fresh sessions, attaches, or replays;
12. P086 admission atomically creates command journal, continuation, work item,
    turn, and reserved side effect; every insert fault rolls back all and
    idempotent replay returns the same IDs;
13. all P086 modes bind target execution/occurrence, use their own configuration
    reservation/receipt, mirror side effects only after prompt CAS, reject fresh
    fallback, and reproduce every old migration phase with an independent
    oracle;
14. Steward claim persists analysis, both internal lane owners, and initial
    turn before provider I/O; its sole retry preserves terminal zero-send turn
    `0`, atomically allocates turn `1`, and rejects a second retry, while
    cancellation before dispatch, during `dispatch_pending`, and after
    `prompt_sent` produce three distinct immutable turn/lane outcomes;
15. every prompt CAS committed result
    `Applied|AlreadyMatching|Conflict|Missing` crosses every
    `DbWriterAcknowledgementV1` case; known `Missing` is never confused with
    unresolved acknowledgement `Unknown`;
16. every registered Class A operation crosses committed, busy/shutdown rejected,
    failed-before-start, uncertain-after-start, commit-before-ack, reconciliation,
    delayed commit after an empty immediate read, caller cancellation, supervisor
    takeover, writer crash, and restart for every owner kind; no request-scoped
    task owns final reconciliation and no mutation is resubmitted;
17. a manager task retains the non-cloneable generation guard through terminal
    response, receipt, active-owner/collateral settlement, and cleanup; compile
    and call-graph tests prove no coordinator second-settlement or callback cycle;
18. a transport that never completes write/flush and every broker/toolchain/
    authority/cleanup timeout remain bounded; no public raw close/kill/cancel or
    prompt API bypass exists;
19. Claude, Gemini, Auggie, and Junie advance the shared prompt ledger while
    keeping provider-configuration truth non-applicable;
20. two owners sharing one generation cross cancellation before/after permit,
    owner interruption, generation closure, and every ordinary/P017/P058/P079
    repair/P079 fallback/P086/Steward collateral row with exact session, attach,
    replay, and prompt counts;
21. launch-barrier/process-binding crashes at every boundary cover original,
    P079 repair/fallback, all continuations, and both Steward lanes; only
    identity-matched processes are reaped;
22. run-wide and scoped invalidation races prove epoch/token fencing and that the
    active manager, not the invalidation caller, performs terminal settlement;
23. real two-process file-DB startup proves one lower-layer lock acquisition,
    immutable source snapshot/fence, deterministic takeover, full legacy matrix,
    process reconciliation, and consumer closure until preflight completes;
    failed preflight returns a `FailedBootstrapOwner` retaining the lock with no
    pool, stays in `failed` without in-process retry, and only restart may recover;
24. the generated replay manifest covers every enqueue/claim/requeue/prompt site
    and rejects unresolved, quarantined, stale, absent, or migration-pending
    authority;
25. all ten producer IDs, including typed P079 fallback, match independent
    compiled/provenance golden vectors across retry, replacement, loop,
    dynamic-idempotency, P058 tier change, and legacy migration;
26. occurrence allocation commits allocator, immutable occurrence row, envelope,
    and work item before visibility; crash before/after enqueue and before claim
    proves claim only copy-validates and creates the first AgentExecution;
27. static, owner, dynamic, and legacy source allocation produces unique
    stage-scoped `human_source_ordinal` values; legacy occurrence backfill is
    invariant under shuffled queries, duplicate timestamps, staged crash/restart,
    and repeat startup, then seeds both occurrence and human-source allocators at
    verified `max + 1`;
28. canonical JSON/timestamp/receipt fixtures cover duplicate-key rejection,
    digest mismatch, exact tagged nullable time ordering, every link state,
    schema-version decode, mutation negatives, and byte-identical v1
    `receipt_json` plus existing report/MCP projection before/after migration;
29. migration 095 remains byte-identical and checksum-equal; migration 100
    survives every staged interruption, maps every 095 lease/fallback row
    exactly, routes dangling mandatory identities only to typed quarantine,
    proves canonical-plus-quarantine count/digest equality and disjoint keys,
    and never drops or merges history;
30. direct-SQL P079 negatives reject active zero selected budget, contradictory
    flags/provenance/source schema, budget mutation, wrong work-item kind, and
    cross-operation turn/lease references; direct-SQL negatives also include
    dispatch-unknown without start, terminal operation with active lease/item,
    incomplete slot/link, and mismatched greatest-attempt result, while accepted,
    rejected, unavailable, cancelled, and superseded validation settlement
    atomically close item/lease/operation/event/parent/artifact truth;
31. direct-SQL P058 negatives reject every cross-ledger/run/stage/agent/tier/
    attempt/policy tuple plus all update/delete attempts against immutable prompt
    authority;
32. fresh `AppSchema::sdl()` byte-matches the checked-in snapshot, every new enum
    literal is lowercase snake case, and uppercase/mixed/unknown values fail;
    both exact
    schema-version literals, simultaneous original-receipt-A/P079-repair-
    receipt-B on physical generation A and target-generation-A/P086-
    continuation-generation-B turn truth, and the exact `P031RunDetail`,
    `P031RuntimeStatusChanged`, and `P031TimelineRawDetail` snapshots from the
    three shipped `P031GraphQLDocumentSet` properties execute against real
    resolvers and decoders;
33. same-agent interleaved execution events preserve execution, occurrence,
    sequence, presentation-row, event, and lane identity through DB, GraphQL,
    Swift, the mandatory occurrence-presentation join, filtering, expansion,
    copy, and swapped-handle rejection; missing/duplicate/cross-run join rows fail,
    while null-row stage/legacy events remain only in deterministic `Run events`;
34. historical and v2 event fixtures preserve the old `runtime_event_id`
    output for identical old inputs, while every `available`, `missing`,
    `stale`, `unauthorized`, `unavailable`, and `digest_mismatch` raw-detail
    result obeys the exact raw/error/identity nullability matrix;
35. topology association and layout fixtures cover all source kinds, legacy
    unique/ambiguous, complete occurrence-scoped attempt lists, transition
    identity, SCC/median/track/virtual/self-loop rules, shuffled input, stress,
    mixed heights, and topology-unavailable recovery;
36. formatter goldens cover every legal configuration/evidence/delivery state,
    including option invalidation, every known/unknown model and effort,
    byte-distinct cancellation before acceptance versus after verified
    acceptance, every exact failure/repair/continuation/historical string,
    canonical ASCII ordinals under multiple host locales, compact bounds,
    Help/copy/accessibility values, and illegal tuple rejection;
37. the single `RunOccurrenceSelectionReducer` covers run change, planned-to-
    current replacement, retained previous-row metadata, next/preceding/heading
    fallback, exact tagged surface/control-qualified occurrence/event/heading/
    run-events/empty targets, retries, and same-agent occurrences; the
    generation-qualified publication owner drops delayed run-A responses after
    run B without any state mutation; no view-local agent-ID selection or
    `activeTimelineAgents.first` remains;
38. accessibility fixtures include the exact human planned, sequenced, and
    legacy occurrence discriminator in row/control labels, contain no digest or
    UUID in spoken strings, and keep static/dynamic/repeated tasks byte-distinct
    through unique human-source and occurrence ordinals; hosted `NSHostingView`
    tests assert the actual `.focused` binding and first responder, not only
    reducer state and not physical keyboard or VoiceOver event delivery;
39. one prebound tri-state router owns the listener once; bootstrap transitions
    `starting -> normal|failed`, one `FirstFatalCoordinator` closes both fences,
    persists one immutable reason under the commit barrier before notification,
    and moves `normal -> failed`; later reasons cannot replace it, normal routes/
    consumers close, every paused writer rolls back, and persistence failure
    exits without reopening; the minimal GraphQL handler accepts only the exact
    Operator `DaemonStatus` AST, rejects mixed/aliased/fragmented/variable/batch
    forms, and performs zero DB accesses in starting and failed states;
40. `codex-model-truth` executes the exact shared function/test-array content of
    `proposal-027`, `proposal-058`, `proposal-075`, `proposal-079`, and
    `proposal-086` without recursive shell invocation; the composed gate and
    all five standalone aliases pass on one clean recorded `HEAD`; and
41. structural mutation tests independently remove an operation registration,
    reservation uniqueness, option-update parser, P079 provenance join, P058
    immutable key, occurrence copy check, production GraphQL selection,
    Timeline identity field, selection owner, or scope exclusion, and require
    the owning gate leg to fail.
The gate must fail independently when either Swift result bundle reports zero
tests or omits a required identifier. No network, daemon, live provider, or
remote UI host is required. Release evidence records successful
`codex-model-truth`, `proposal-027`, `proposal-058`, `proposal-075`,
`proposal-079`, and `proposal-086` invocations from the same clean committed
tree and includes their common `HEAD`.

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

- [ ] Fresh compilation freezes the approved seven-profile matrix and exact
      per-profile capability map; frozen pre-change snapshots retain
      `legacy_best_effort_v0` bytes and adapter behavior.
- [ ] Codex negotiation consumes each response/update snapshot in order,
      verifies final model/effort, persists its option-snapshot revision/digest,
      and exposes no permit until durable acceptance succeeds; Claude alias
      matching is unchanged.
- [ ] Every changed or malformed `config_option_update` before/during prompt
      durably invalidates generation acceptance with the exact zero-send or
      unknown-delivery result, blocks stale reuse, and cannot be rendered as
      actual runtime identity.
- [ ] Requested, accepted, and invalidated truth is nullable, versioned,
      owner-scoped, generation-bound, and tied to a stable task occurrence;
      planned/requested values are never promoted to accepted truth.
- [ ] Enqueue atomically allocates occurrence sequence and writes allocator,
      immutable occurrence row, validated envelope, and work item. Claim
      copy-validates that tuple before creating an AgentExecution; crash between
      enqueue and claim cannot allocate again.
- [ ] New-generation and existing-generation reservations are separate Class A
      operations. One prompt turn has one globally unique generation binding,
      existing reuse consumes exactly one owner receipt-attempt but does not
      allocate a physical generation, idempotent replay consumes no new index,
      and every race loser performs zero provider/process/prompt I/O. Binding
      state enforces the exhaustive pre-receipt/receipt/failure reference matrix
      and exact owner/attempt/generation correlation.
- [ ] A physical generation has one lifecycle custodian and many sequential
      logical owner bindings. `AcpRuntimeManager` is the sole terminal owner
      after admission and retains the guard through response, receipt,
      active/collateral settlement, and cleanup; the coordinator cannot settle a
      second time or receive a callback.
- [ ] DbWriter acknowledgement certainty is separate from committed domain
      outcome. Every operation handles committed, rejected-before-start,
      failed-before-start, and uncertain-after-start; CAS `Missing` is a known
      domain result and unresolved acknowledgement `Unknown` never permits I/O.
      An uncancellable daemon supervisor owns late-commit reconciliation after
      request return/cancellation, never resubmits the mutation, and either
      proves a result/rollback or closes first fatal.
- [ ] Every new runtime mutation is registered in the P075 operation registry,
      uses a natural idempotency key/result digest, reconciles commit-before-ack,
      obeys shutdown admission/terminal allowlists, and converges after writer
      crash/restart.
- [ ] One `FirstFatalCoordinator` is the sole close authority. It closes prompt
      and mutation admission under the shared commit barrier, durably persists
      exactly one reason/epoch, and only then publishes failed state; no competing
      CAS/watch owner exists. One prebound router transitions
      `starting -> normal|failed` and `normal -> failed`; failed bootstrap retains
      its `PreflightLockGuard` without a runtime pool or in-process retry. The
      minimal Operator GraphQL path accepts only the exact `DaemonStatus` AST and
      performs zero DB accesses. `persist_fatal` failure cannot reopen service.
- [ ] Session lineage, generation, process binding, launch barrier, PID/start
      identity, and bounded cleanup support run-agent, P086-continuation, and
      Steward-lane owners without signalling an identity-ambiguous process.
- [ ] `provider_prompt_turns`, not terminal receipts or owner-domain rows, is
      sole dispatch authority. Initial/final CAS, byte certainty, unknown
      quarantine, cancellation, and restart cover ordinary, P017, P058, P079
      repair/fallback, P086, and Steward prompts.
- [ ] Migration 095 remains byte-identical. New migration 100 owns complete P079
      v2 DDL and exact 095-source mapping, stages/restarts without row loss or
      merging, routes dangling mandatory identities only to typed quarantine
      with canonical-plus-quarantine count/digest proof, and enforces selected
      budget, provenance, source schema/key, work item, turn, operation, attempt,
      active/terminal state, and timestamp constraints through direct-SQL negatives.
- [ ] P079 repair uses `OutputContractRepair`; its fallback child remains an
      `InvokeAgent` item but carries typed `production.p079_fallback`
      provenance and `p079_fallback_child` prompt ownership. Its initial permit
      joins operation/attempt/lease/parent/binding authority, and collateral loss
      permits zero transparent fresh session, attach, or replay. Original and
      repair turns use distinct receipt attempts on the same physical generation;
      post-validation settlement atomically closes item, lease, operation, event,
      parent/transition, and artifact publication or quarantine.
- [ ] P058 prompt authority is an immutable complete
      execution/ledger/run/stage/agent/tier/kind/attempt/policy tuple. Composite
      FK plus insert/update/delete negatives prevent cross-ledger use or
      post-reservation tier mutation.
- [ ] P086 admission atomically commits command, continuation, item, turn, and
      side effect plus immutable resume-context ID/digest. Resurrection launches
      and binds process identity before checking advertised ACP resume capability,
      uses only exact frozen `session/resume` inputs, rejects pre-response option
      updates without reordering, and requires a correlated complete response
      option catalog before set/readback. Every post-launch failure reaps by
      identity. Every mode owns configuration evidence,
      preserves target execution/occurrence, rejects fresh fallback, and passes
      the exhaustive old phase/release migration oracle.
- [ ] Steward persists analysis and both internal lane owners before provider
      I/O, uses no synthetic run/execution, applies one total delivery reducer
      including `steward_pending|steward_sent|unknown`, preserves failed turn `0`
      before allocating retry turn `1`, and distinguishes cancellation before
      dispatch, during dispatch, and after prompt sent. Both lanes settle with at
      most one durable zero-send retry.
- [ ] The closed collateral matrix covers ordinary, P017, P058, P079 repair,
      P079 fallback, P086, and Steward. A sent collateral turn is never replayed;
      only the explicit ordinary policy may allocate a fresh generation after
      closure.
- [ ] Startup keeps consumers closed until migration, receipt/invalidation,
      process, turn, quarantine, and owner reconciliation complete. Generated
      replay selectors reject every unresolved or unclassified path.
- [ ] The ten exact production producer IDs, including typed P079 fallback,
      delegate to sealed envelope enqueue/claim validation; legacy migration is
      separate, and compiled/occurrence identity survives retry/fallback without
      agent-ID reconstruction.
- [ ] Legacy occurrence backfill uses the immutable tagged total order, resumes
      at every staged boundary, is query-order independent, and seeds runtime at
      verified `max + 1`; underivable pending rows fail and terminal ambiguity
      remains nullable.
- [ ] Canonical codecs, receipts, option snapshots, identities, timestamps, and
      nullability have independent known-answer fixtures, digest/duplicate-key
      checks, exhaustive legal rows, and mutation negatives.
- [ ] `AppSchema::sdl()` and schema probe are exact. The production
      `P031RunDetail`, `P031RuntimeStatusChanged`, and `P031TimelineRawDetail`
      operations from the exact shipped `P031GraphQLDocumentSet` properties
      request every occurrence/truth/attempt field; shipped DTOs distinguish
      omission from null and no query selects execution truth by agent ID.
      Execution and turn schema-version literals are exact, and turn-owned
      configuration readback simultaneously preserves original and P079/P086
      specialized generation truth. Every new enum literal is explicit lowercase
      snake case; uppercase, mixed-case, and unknown values are rejected.
- [ ] Existing MCP `2024-11-05`, `run://`, `report://`, `reports.get`,
      tools envelopes, `steward.list_analyses`, `steward.get_analysis`, generated
      reports, artifact bytes, provider filesystem grants, and adapter containment
      remain unchanged. Structural tests reject accidental aliases or
      protocol/report/materializer/filesystem expansion in this slice.
- [ ] `AcpRuntimeReceipt` remains schema v1. Migration preserves every
      `receipt_json` byte, stores prompt/configuration correlation only in
      private relational columns/tables, and byte-compares existing Operator,
      Agent, and Observer report/MCP projections before and after migration.
- [ ] GraphQL and Swift distinguish planned, configuring, configured,
      invalidated, prompt-pending/sent/unknown, failed, cancelled, and legacy
      states. One formatter owns visual, Help, copy, and accessibility values and
      never renders invalidated or planned values as actual. Cancellation before
      acceptance and after verified acceptance have byte-distinct copy and legal
      tuple predicates.
- [ ] Timeline and raw-detail identity preserve exact event, execution,
      occurrence, sequence, presentation-row, and lane tuples through DB,
      GraphQL, shipped Swift DTOs, the mandatory occurrence-presentation join,
      filtering, expansion, and copy. Missing/duplicate/cross-run joins fail. The old
      `rte_` algorithm/handles remain byte-compatible; all six raw-detail status
      rows obey exact raw/error/identity nullability; null-row events remain in
      the separate deterministic `Run events` lane.
- [ ] One `RunOccurrenceSelectionReducer` owned by
      `P031RunsHomeViewModel` governs Overview, Stages, Timeline, Run Inspector,
      popover, and focus. It retains prior selected-row metadata across row-array
      replacement and uses exact tagged, injective surface/control-qualified
      targets. A generation-qualified publication owner drops every stale run
      response/update without mutating the newer run. It has no per-run
      dictionary, view-local agent selection, or
      `activeTimelineAgents.first` fallback.
- [ ] Every row/control accessibility value includes the exact planned,
      sequenced, or legacy human occurrence discriminator and no digest, UUID, or
      provider-session ID. Static, dynamic, and legacy source ordinals are unique
      and rendered as locale-independent ASCII decimal. Opaque row identity
      remains only in the machine accessibility identifier. Pure/hosted tests
      prove exact formatter strings plus real `.focused`/first-responder targets;
      this proposal does not claim local proof of physical
      keyboard or VoiceOver event delivery.
- [ ] `./scripts/test-gate.sh codex-model-truth` runs nonzero Rust and
      independently nonzero pure/hosted Swift suites and the exact shared legs
      from P027/P058/P075/P079/P086. The composed gate and all five standalone
      aliases pass on one clean committed `HEAD` with no relevant untracked
      implementation path.
