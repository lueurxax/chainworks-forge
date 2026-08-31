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
- Move P079 repair output into runtime-owned operation staging and publish it
  through crash-consistent, inode-independent history/canonical activation.
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
  output schemas, or generated run-report bytes. Those surfaces retain their
  current wire and authorization behavior through the explicit compatibility
  projections in this proposal and continue to expose only their existing
  planned/requested fields.
- Change artifact materialization or provider filesystem policy outside the
  P079 repair/fallback lane. The P079 staging, history, canonical activation,
  and least-privilege repair-output grant are an explicit in-scope safety
  redesign; ordinary provider output and every non-P079 artifact path remain
  unchanged.
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

Fallback target selection is itself frozen authority, not an adapter default.
Fresh catalog YAML uses this duplicate-key-rejected exact shape:

```yaml
provider_fallback_policy:
  schema_version: provider_fallback_policy_v1
  routes:
    - route_id: codex_builder_transport_to_claude
      source_backend_profile_id: codex_builder_high
      reason: provider_transport_unavailable
      priority: 100
      target_backend_profile_id: claude_builder_high
```

`ProviderFallbackReasonV1` is closed to
`configuration_unavailable | provider_start_failed |
provider_transport_unavailable | provider_runtime_failed |
provider_runtime_timeout | output_contract_repair_fallback`. Every route has a
unique non-empty ASCII `route_id`, an existing distinct source and target
profile, checked `priority` in `0...65535`, and exactly one listed reason; no
wildcard, provider-only selector, implicit catalog order, or adapter fallback is
legal. Fresh compilation rejects duplicate route IDs and two routes with the
same `(source_backend_profile_id, reason, priority)`. It canonical-sorts routes
by `(source_backend_profile_id, reason, priority, route_id)`, freezes the complete
canonical JSON plus `provider_fallback_policy_sha256` into the run snapshot, and
records that digest and selected route ID in `EffectiveProviderContractV1`.

`FrozenProviderFallbackPolicyV1::select(source_profile_id, reason)` reads only
that snapshot. Zero matches returns typed `fallback_route_not_found`; more than
one match at the lowest priority is typed `fallback_route_ambiguous`; exactly
one lowest-priority match returns its target and binding digest. Neither failure
enqueues work or consumes a fallback budget. Current catalog edits, YAML route
order, renamed defaults, or adapter/provider preference cannot affect replay.
Rust `ProviderFallbackPolicyV1/ProviderFallbackRouteV1`, the frozen JSON schema,
and read-only Swift `P031ProviderFallbackPolicyReadModel` share generated closed
enums and presence-aware decoding. Known-answer vectors cover no match, one
match, same-priority ambiguity, different-priority precedence, YAML reordering,
duplicate keys/IDs, missing target, self-target, current-catalog drift, and
frozen replay. P079 controlled fallback and ordinary compatibility fallback use
this sole selector before constructing the effective contract.

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

`InvokeAgentEnvelopeV1` requires run ID, owner kind/ID, compiled-task ID, a
sealed `InvokeSourceProvenanceV1`, captured run dispatch epoch, per-attempt
effective provider contract, and the existing provider, agent, session-reuse,
and payload fields. Its invocation identity is the required tagged union
`InvokeAgentOwnerIdentityV1`, never a bag of independently nullable IDs:

| Identity branch | Required fields | Forbidden fields |
|---|---|---|
| `occurrence_bound` | non-null task-occurrence ID and source-scoped occurrence sequence; stage execution when the durable owner is stage-bound | mediation execution ID |
| `p017_mediation` | non-null mediation record ID and mediation-owned AgentExecution ID | stage-execution ID, task-occurrence ID, occurrence sequence |

Only producer `orchestrator.p017_mediation` may construct the second branch,
and that producer may construct only that branch. The factory derives the
applicable identity before the queue row becomes visible; the claim path
recomputes and validates the complete tagged tuple against durable owner truth
before creating or reusing an `AgentExecution`. P017 therefore has a compiled
task coordinate and a mediation-owned execution, but it never allocates,
backfills, hashes, or persists a synthetic task occurrence. Every other
production producer is occurrence-bound.

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
or locale folding is performed. For the `occurrence_bound` branch,
`task_occurrence_v1:<sha256>` uses the same codec with domain
`chainworks.task_occurrence.v1` and ordered components `owner_kind`, `owner_id`,
`compiled_task_id`. The P017 branch does not invoke this codec and requires the
envelope JSON keys `stage_execution_id`, `task_occurrence_id`, and
`occurrence_sequence` to be explicit null; omission, a non-null value, or an
occurrence allocator/hash call is `invoke_agent_identity_invalid` before
enqueue.

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
- opaque provider-session ref ID and binding fingerprint;
- accepted-at timestamp;
- bounded `ProviderConfigurationAcceptanceV1` and its SHA-256 digest.

Before a reused prompt, the engine submits
`provider_configuration.reserve_existing_generation`. Its atomic transaction
loads the active generation and requires generation ID, opaque provider-session
ref ID, provider, binding fingerprint, contract version, and live-handle/process
identity to match. For `codex_exact_pair` it additionally requires requested
model/effort and a still-valid `GenerationOptionSnapshotV1` revision/digest. For
`provider_neutral|legacy_best_effort` it instead requires the unique immutable
class-matching readiness row and forbids accepted model/effort. It also requires the
configuration owner's active attempt/generation fields to be null, reads
`next_configuration_attempt_index = n`, and inserts exactly one logical owner
binding for the already-created generation and prompt turn with attempt `n`.
It does not allocate a generation or advance the physical session-generation
allocator.

When reservation commits it advances `next_configuration_attempt_index` to
`n + 1`, leaves the active pair null, and leaves the new binding `admitted` with
no success evidence. The class-specific settlement then runs before permit:
`provider_configuration.settle_success` derives a new owner-bound
`ProviderConfigurationReceiptV1` from exact generation acceptance, while
`provider_configuration.settle_readiness` binds the immutable physical
readiness to a non-exact owner/attempt/turn. For a run-agent exact owner, success
also writes the receipt projection to the new `AgentExecution` with
`acceptance_source = reused_session_generation` and the source acceptance
digest. That derived receipt names the new agent execution and task occurrence;
it does not copy the first execution's IDs. P017 mediation, P079 repair, P086,
and Steward settlement instead applies their closed nullable owner branch and
never fabricates an occurrence or execution. Both results are inherited
response-correlated authority from the same live provider session, not a new
negotiation. A crash after reservation but before settlement leaves an admitted
zero-send binding that startup settles by the same registered operation; it
cannot dispatch from source generation evidence alone.

`provider_generation_owner_bindings.prompt_turn_id` is globally unique, not
merely unique within a generation. Its active-owner key is also unique on
`(prompt_owner_kind, prompt_owner_id, prompt_turn_id)`. A prompt turn can
therefore reserve either one new generation or one existing generation, never
both. The binding stores configuration owner kind/ID, allocated attempt index,
provider contract class, and the mode-dependent receipt/readiness/failure/
cancellation/post-outcome refs from the exhaustive matrix below. Idempotent
successful replay with the same generation returns that existing binding,
receipt, and attempt index without advancing the allocator; the same
turn with another generation is `Conflict`. Races between reuse,
invalidation, cancellation, and new-generation fallback are resolved in that
single reservation transaction before any process/session/prompt I/O.

When evidence is absent, stale, malformed, or mismatched, the manager closes
and invalidates the generation before any prompt. Only an original InvokeAgent
whose frozen ordinary-owner policy explicitly allows compatibility recovery may
then perform at most one fresh-session fallback through the complete negotiation
transaction. P079 repair and P086 fail closed because they require their exact
contained/attached generation; P079 fallback may use only its separately
admitted contained fresh generation. Steward also has no transparent fallback: the current turn
fails zero-send and only the lane's explicit one-retry authority may allocate
turn `1`, after which the common configuration allocator creates a fresh
generation. The old session receives zero prompts.

P086 provider-session resurrection never copies acceptance from the source
daemon generation into a newly attached generation. The current
`ProviderAttachProtocolV1` has exactly one supported member,
`claude_session_new_resume_session_id_v1`, preserving the already-shipped
Claude `session/new.params.resumeSessionId` contract. Codex, Gemini, Auggie,
Junie, and an unknown adapter are unsupported and fail before launch. In
particular, this proposal does not promote the observed-but-unproven Codex
`session/resume` shape into authority. Adding a Codex member requires a later
schema-version bump plus a checked-in `ProviderAttachConformanceManifestV1`
from the exact pinned executable: create a session, terminate its child,
restart the adapter, resume the exact private session, return a correlated
complete option catalog, reject a mismatched session, and prove zero prompt on
every mismatch. Unknown executable versions remain unsupported. Before the source generation
can become resumable, the adapter
persists secret-safe `ProviderSessionResumeContextV1` with schema version,
context ID, source generation ID, an opaque `provider_session_ref_id`,
provider/adapter contract version, exact attach-protocol tag, target binding
fingerprint, ordered `ProviderRootAuthorityV1` set for `cwd` and
`additionalDirectories`, and an immutable MCP
descriptor-set reference plus RFC 8785 digest. The descriptor set contains
names/transports and references to broker-owned secret inputs, never raw tokens,
expanded environment secrets, or a northbound provider-session ID. The source
generation stores the context ID/digest; P086 admission copies both into the
continuation in the same transaction as its frozen target binding. A
missing/mismatched context or digest rejects admission before process I/O.

Every `cwd` and additional root in that context is a
`ProviderRootAuthorityV1`, not a path string. The descriptor proves identity,
not access control: every resurrection additionally requires
`P086FilesystemContainmentV1`. Admission walks from a trusted
workspace/root directory FD with no symlink traversal, opens the directory, and
records canonical display path plus `st_dev`, `st_ino`, `st_gen`,
`st_birthtime_ns`, `fstatfs.f_fsid`, mount flags, and the required read/write
mode. The ordered root-authority records and common-codec digest are immutable
members of the resume context. At launch the runtime reopens each original path
by no-follow dirfd traversal and requires the complete identity byte-equal; then
it passes the held descriptors into the verified child. The child cwd is set by
descriptor (`posix_spawn_file_actions_addfchdir`), and ACP-visible cwd/
`additionalDirectories` use only inherited `/dev/fd/<n>` aliases tied to those
descriptors. Original path strings are display evidence only and are never
re-resolved by the provider attach path. Before launch, the runtime compiles the
frozen root mode into a mandatory inherited macOS Seatbelt profile. It denies
write, rename, unlink, link, clone, create, truncate, xattr, chmod/chown, and
executable creation everywhere by default; a `read_only` authority grants only
read/search against its exact descriptor-bound vnode, while a `read_write`
authority grants only the explicitly frozen mutation set under that root. Roots
absent from the context are inaccessible. If the OS cannot bind the profile to
the reopened identity, admission is zero-send
`resume_filesystem_containment_unavailable`; the directory FD alone is never a
permit. External HTTP/SSE MCP and host Xcode brokers are removed for this
contained attachment, and any allowed stdio helper is launched inside the same
profile from the verified launch closure. Same-path directory replacement,
symlink retarget, mount/fsid change, inode generation change, descriptor loss,
or reordered/duplicated roots rejects zero-send. Restart may reopen only an
exact durable identity; it never substitutes the current path target. Race
fixtures replace each directory before and during launch and prove no attach,
prompt, or widened root access. Syscall fixtures execute `openat(O_WRONLY)`,
rename, hard/symbolic link, clone, truncate, chmod, xattr, and descendant-helper
variants against read-only, read-write, sibling, daemon-control, and canonical
roots and require the exact frozen matrix.

The supervised child is then launched and bound to target process identity.
For Claude, the verified launch image and
frozen adapter capability fingerprint must declare the exact
`session/new.params.resumeSessionId` contract, after which the adapter sends
exactly one `session/new` carrying `resumeSessionId`; no generic initialize
resume bit is invented. That request is populated only from the admitted
immutable context: the internally resolved stored provider session ID, exact
descriptor-bound `cwd`, complete ordered descriptor-bound additional roots, and
complete frozen MCP descriptor set after broker-side secret resolution.
Initialize/contract failure is `ACP_P086_RESUME_UNSUPPORTED`, followed by
bounded identity-safe reap and zero prompt bytes. `session/resume`,
`session/load`, the wrong provider branch, an untagged `session/new`, omitted
roots/MCP, admission-time
recomputation from the current workspace, or any adapter-private attach method
is forbidden.

The correlated tagged attach response must arrive for that request and must
contain a non-empty, completely parseable `configOptions` array. Although ACP
makes this member optional generally, P086 exact-pair attachment makes it
required. Claude requires non-empty `result.sessionId` resolving to
the same private provider-session identity. The response seeds
`GenerationOptionSnapshotV1` at local revision
zero. An authority-bearing `config_option_update` observed after the attach
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

After this catalog source is established, the manager loads the continuation's
already-reserved active configuration attempt/generation/process intent and
runs the normal model-first set/readback sequence against the resumed session.
Claude's one tagged attach `session/new` is the sole allowed occurrence. It
never allocates or substitutes an owner tuple after
launch. Response-verified equality may create the new generation
acceptance and owner receipt with
`acceptance_source = attached_session_reverification`. The attach receipt,
active attempt, new generation, process binding, acceptance, option snapshot,
and continuation turn form one authority tuple before a permit. If the provider
cannot re-read and confirm both options, attachment fails zero-send; old
generation evidence is never transferred by ID or digest. Fake ACP fixtures
cover missing/mismatched admitted context, launch/initialize/capability failure,
tagged attach error, omitted/empty/partially invalid options, session mismatch,
pre-response update rejection, ordered post-response update, invalidation
during both set calls, and the accepted exact pair. Every post-launch negative
asserts identity-safe reap; every negative asserts no `session/prompt` or
`session/load`, and no `session/new` except the one expected Claude attach
request, with no source acceptance transfer. Existing Claude continuation
fixtures remain byte-compatible rather than being retired implicitly.

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
| `provider_configuration_state` | `configuring`, `configured`, `invalidated_after_acceptance`, `failed_before_prompt`, `cancelled_before_configuration`, `configured_terminated_before_prompt`, or `legacy_unverified`; `null` for non-Codex |
| `provider_configuration_verified_at` | Complete-pair verification time; otherwise `null` |
| `provider_configuration_invalidated_at` / `provider_configuration_invalidating_snapshot_sha256` | Durable option-update invalidation evidence; otherwise `null` |
| `provider_configuration_receipt_json` / `provider_configuration_receipt_sha256` | Bounded projection of the authoritative owner-scoped receipt and its verified digest |
| `acceptance_source` | `fresh_negotiation`, `reused_session_generation`, or `attached_session_reverification`; otherwise `null` |
| `configuration_evidence_state` | Non-null `pending`, `receipt_available`, `readiness_available`, `invalidated`, `failure_available`, `cancellation_available`, `not_applicable`, or `legacy_unverified` |
| `next_configuration_attempt_index` | Non-null monotonic allocator, initialized to `0` |
| `active_configuration_attempt_index` / `active_configuration_generation_id` | Nullable pair reserved atomically before provider launch; at most one pair exists per owner |
| `current_provider_configuration_receipt_id` | Nullable FK to the latest successfully persisted owner receipt |
| `current_provider_readiness_id` | Nullable FK to the physical non-exact readiness currently bound to this owner/attempt/turn |
| `current_provider_configuration_failure_id` | Nullable FK to the latest typed pre-acceptance failure; mutually exclusive with receipt/cancellation pointers |
| `current_provider_configuration_cancellation_id` | Nullable FK to the latest typed pre-acceptance cancellation; mutually exclusive with receipt/failure pointers |
| `current_provider_post_configuration_outcome_id` | Nullable FK allowed only with the same attempt's class-appropriate receipt/readiness after zero-prompt termination |
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
- creates `provider_configuration_failures` for every non-cancellation terminal
  zero-send configuration failure, including receipt-persistence failure after
  rolling back the uncommitted receipt;
- creates separate append-only `provider_configuration_cancellations` only for
  `cancelled_before_configuration`, with no receipt/failure row;
- creates append-only `provider_post_configuration_outcomes_v1` only for a
  class-appropriate verified receipt/readiness whose turn remains `not_started`,
  with no failure/cancellation row or `AcpRuntimeReceipt`, plus append-only
  `provider_configuration_cleanup_events_v1` for cleanup progression;
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
`current_provider_configuration_receipt_id`,
`current_provider_readiness_id`,
`current_provider_configuration_failure_id`, and
`current_provider_configuration_cancellation_id` plus nullable
`current_provider_post_configuration_outcome_id` FKs, and non-null
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
analysis inputs and calls the sealed Class A `steward_analysis.claim`. Its one
transaction inserts the `steward_analyses` row as `running`, inserts both lane
rows as `reserved`, binds the already claimed StewardAnalysis work item, and
allocates only the system lane's turn `0`. A `reserved` lane with a committed
turn and null active configuration attempt/generation is the exact legal
claim-before-reservation state; claim does not create a generation. The common
registered `provider_configuration.reserve_new_generation` operation then uses
the sealed Steward-initial permit to create exactly one tuple for that turn.
Startup re-enters that same operation by turn ID after a claim-before-reservation
crash, so replay or competing workers cannot allocate twice. The auditor lane remains turnless
until a validated system health report permits the sealed Class A
`steward_auditor_lane.activate`, which allocates only auditor turn `0` and
enters the same legal null-generation reservation state.
Provider calls happen only after the corresponding operation commit. Final
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

The only automatic Steward replay is a zero-send infrastructure retry. The two
sealed initial operations above allocate their respective turn index `0`; each
permit requires
`zero_send_retries_consumed = 0`, exactly that active turn, and no earlier turn.
An eligible typed infrastructure failure leaves turn `0` permanently
`not_started` with its non-null failure code, appends matching
`provider_configuration_failures` cleanup evidence, proves the process reaped
or never launched, and moves the lane to `zero_send_retry_pending`. It does not
return a terminal lane directly to `reserved`.

One Class A `steward_lane.retry_zero_send` transaction may then CAS
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
any committed configuration receipt, or failure to prove cleanup settles through
the table above and cannot requeue. A Steward generation that committed accepted
configuration but lost transport before dispatch retains receipt A and
terminalizes the lane as
`configuration_failed(configured_transport_lost_before_dispatch)` with turn `0`
still `not_started`; it consumes no retry and never allocates turn `1` or another
generation. Only a no-receipt configuration-failure row can enter
`zero_send_retry_pending`.
Crash fixtures stop before and after failure settlement, retry counter CAS,
new-turn insert, launch barrier, cancellation in each dispatch state, and lane
terminal settlement and prove one retry and at most one prompt write.

Fresh-generation policy is closed: an ordinary initial owner may use only its
frozen ordinary recovery allowance; Steward turn `0` gets one generation and
the proven zero-send retry turn `1` gets one different generation; P079 repair
gets exactly one atomically admitted contained generation attached to the source
provider session, while P079 fallback gets exactly one contained fresh
generation; and P086 must use its live or provider-specifically attached
generation.
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
not exclusive prompt authority. Session reuse adds append-only
`provider_session_readiness_v1` for successful pre-prompt session readiness
that is not Codex exact-pair acceptance. Cardinality is exactly zero or one row
per physical `(session_generation_id, provider_contract_class)`, enforced by a
unique key. It stores readiness ID, generation/process/session identity,
`provider_contract_class = provider_neutral | legacy_best_effort`, provider,
correlated initialize/session-creation response identity, ready time, and
bounded evidence digest. It contains no logical owner or prompt-turn field, no
accepted model/effort, is not an `AcpRuntimeReceipt`, and rejects update/delete.
`codex_exact_pair` is forbidden from this table and must use a
`ProviderConfigurationReceiptV1` instead. Multiple sequential logical owners
may reference the same immutable physical readiness row, but each reference is
settled independently against that owner's exact attempt and turn as described
below.

Session reuse also adds
`provider_generation_owner_bindings`, keyed by
`(session_generation_id, prompt_owner_kind, prompt_owner_id, prompt_turn_id)`.
It stores the logical owner tuple, work-item ID, occurrence when run-bound,
configuration-owner kind/ID, `provider_contract_class = codex_exact_pair |
provider_neutral | legacy_best_effort`, non-negative configuration-attempt index
(allocated from the owning row's monotonic allocator for every class),
nullable `configuration_receipt_id`, nullable `configuration_failure_id`,
nullable `configuration_cancellation_id`,
nullable `provider_readiness_id`, nullable `post_configuration_outcome_id`,
nullable immutable `configuration_settlement_operation_name` and
`configuration_settlement_journal_key`,
nullable terminal reason, binding state `admitted`, `configured`,
`provider_ready`, `waiting_for_prompt_gate`, `dispatching`, `awaiting_terminal`,
`terminal`, or `cancelled`, and timestamps. A generation may therefore have many sequential
logical owners while each owner/turn belongs to exactly one generation. The
final rebuild installs all five evidence references only after their target tables exist and
enforces this exhaustive matrix:

| Contract / binding result | Receipt | Readiness | Failure | Cancellation | Post-config outcome | Additional rule |
|---|---:|---:|---:|---:|---:|---|
| any `admitted` | null | null | null | null | null | Turn `not_started`; no successful or terminal evidence |
| exact `configured` or dispatch-capable | non-null | null | null | null | null | Receipt owner/attempt/generation/turn exact |
| neutral/legacy `provider_ready` or dispatch-capable | null | non-null | null | null | null | Readiness class/generation/process exact; binding owner/attempt/turn exact; accepted pair remains null |
| exact terminal before acceptance | null | null | non-null | null | null | Typed failure; turn `not_started` |
| exact cancelled before acceptance | null | null | null | non-null | null | Typed cancellation; turn `not_started` |
| neutral/legacy terminal before readiness | null | null | non-null | null | null | Typed provider-start/readiness failure; owner/attempt/generation/turn exact; turn `not_started` |
| neutral/legacy cancelled before readiness | null | null | null | non-null | null | Typed pre-readiness cancellation; owner/attempt/generation/turn exact; turn `not_started` |
| exact terminal/cancelled after acceptance but before prompt | non-null | null | null | null | non-null | Receipt preserved; typed post-configuration zero-prompt outcome; turn `not_started` |
| neutral/legacy terminal/cancelled after readiness but before prompt | null | non-null | null | null | non-null | Readiness preserved; matching `provider_ready_*` zero-prompt outcome; turn `not_started` |
| exact or non-exact terminal after prompt | receipt iff exact | readiness iff non-exact | null | null | null | Turn is `prompt_sent|dispatch_unknown`; closed terminal reason |

The generated provider-class x owner-kind matrix is also normative:

| Logical owner family | Allocator/current-pointer row | Exact success | Neutral/legacy success | Failure/cancellation before success | GraphQL owner projection |
|---|---|---|---|---|---|
| ordinary run agent, P058, P079 fallback child | owning `agent_executions` row | receipt pointer | readiness pointer | failure or cancellation pointer | execution truth and exact turn child |
| P017 mediation | mediation-owned `agent_executions` row, stage/occurrence null | receipt pointer | readiness pointer | failure or cancellation pointer | mediation execution truth and exact turn child |
| P079 repair | exact lease/attempt row | receipt pointer | readiness pointer | failure or cancellation pointer | repair-turn child only; parent pointer unchanged |
| P086 continuation | `agent_work_continuations` | receipt pointer | readiness pointer | failure or cancellation pointer | continuation turn child; target execution unchanged |
| Steward system/auditor lane | `steward_agent_lanes` | receipt pointer | readiness pointer | failure or cancellation pointer | no new public Steward object; internal gate projection only |

Every cell consumes monotonic attempt `n`, binds the same owner/generation/work-
item/turn tuple, and uses one of the registered Class A settlement codecs. SQL
generation emits each legal cell and rejects every cross-class pointer, missing
pointer, wrong owner, and attempt reuse. `configuration_evidence_state` is
`receipt_available`, `readiness_available`, `failure_available`, or
`cancellation_available` for the corresponding settled cell; `pending` is the
only admitted value. `not_applicable` remains only a planned non-exact shell
with no execution/attempt. The generated Rust/SQL/GraphQL fixture has one row
per matrix cell and is the sole source for owner pointer CHECKs and readback
nullability.

The five evidence refs are mode-dependent, not one undifferentiated one-of.
Insert/update triggers reject a receipt/readiness/failure/cancellation/post-
configuration owner, contract class, attempt, generation, work-item, or prompt-
turn mismatch; reject a dispatch-capable state without the class-appropriate
receipt/readiness authority; reject a pre-readiness terminal whose prompt turn
advanced; and prevent a terminal/cancelled binding from becoming active again.
The transition that persists exact acceptance inserts the receipt and moves
`admitted -> configured` atomically. Provider-neutral/legacy session readiness
uses registered Class A operation
`provider_configuration.settle_readiness`: it inserts-or-loads the unique
physical readiness row, then binds its digest to the exact logical owner,
attempt, work item, and prompt turn while moving only that binding
`admitted -> provider_ready` in the same transaction. Same-key replay verifies
both the physical row and logical binding. Another class, generation, process,
owner, attempt, turn, or readiness digest is `Conflict`.
Any class's zero-send failure or pre-success cancellation inserts only its typed
failure/cancellation row, moves that owner's matching pointer, and
terminalizes/cancels the binding. Non-exact rows keep accepted/configuration
fields null but expose the same typed evidence authority. A
partial unique index permits only one `dispatching|awaiting_terminal` binding
per generation. The lifecycle custodian cannot authorize a prompt on behalf of
another binding, and deleting or closing the custodian does not erase the
other durable owner rows.

Before any class's success/failure/pre-success-cancellation settlement starts,
the Class A pending-intent admission atomically CASes the binding's two
configuration-settlement locator fields from null to that operation name/key.
They are immutable thereafter and unique for the owner/attempt; another outcome
or digest is `Conflict`. Thus cleanup/startup always knows the exact receipt/
readiness, failure, or cancellation journal to reconcile. The later
post-configuration-outcome operation has its own deterministic key derived from
that class-appropriate settled evidence and cannot replace the original locator.

`provider_process_bindings` is keyed by session-generation ID and contains the
physical lineage/session custodian tuple, provider, launch-gate PID, eventual
provider PID (the same PID after the gate's final `execve`), process-group ID
where supported, process-start identity, non-null persisted
`VerifiedProviderLaunchImageV1`, immutable macOS `boot_session_id`, parent
daemon PID, daemon-generation ID, and state `spawn_pending`,
`launch_gate_waiting`, `provider_exec_released`, `running`, `exit_observed`,
`reaped`, or `identity_ambiguous`. Before spawn, the supervised launcher inserts
`spawn_pending` with a random launch nonce and expected launch-image digest.
No provider executable is the first image in that process.

macOS launch uses the bundled, hardened-runtime, code-signature-pinned
`chainworks-provider-launch-gate` helper as the exact trusted bootstrap barrier.
The daemon starts only that helper with `POSIX_SPAWN_START_SUSPENDED`,
`POSIX_SPAWN_CLOEXEC_DEFAULT`, an empty environment, no workspace/provider/MCP
descriptor, and only three fixed descriptors: read-only verified-launch-root,
one-shot release pipe, and status pipe. It verifies the suspended helper's code
signature, executable identity, PID/start identity, boot session, parent, and
absence of `DYLD_*`/injection state before changing the binding to
`launch_gate_waiting`; only then may it resume the helper. The helper itself is
part of the daemon trust boundary, loads no plugin/provider code, installs a
baseline Seatbelt profile denying filesystem mutation and network, and blocks
on the release pipe before opening or executing the provider image. EOF,
malformed data, timeout, or a release nonce/digest mismatch exits without
provider `execve`.

`VerifiedLaunchReleasePermitV1` is non-cloneable and can be derived only after
the `launch_gate_waiting` binding commit is read back byte-for-byte. It binds
the launch nonce, binding ID, gate PID/start identity, boot session, complete
launch manifest, exact argument template, and exact environment-key allowlist.
The daemon revalidates the gate and private launch root immediately before
consuming the permit. The gate independently reopens and hashes the target (and
the separately declared interpreter plus script when applicable), validates
the release envelope and allowlisted environment, then performs the sole final
`execve`. The PID and start identity remain stable across that operation. A
constructor-marker provider fixture is therefore inert until the binding commit
and explicit release; target/interpreter/script/injection mismatch yields zero
provider code, prompt bytes, tool access, or secret access. After the gate
reports successful `execve`, the daemon verifies `proc_pidpath`, PID/start
identity, code signature, private inode set, and manifest again before moving
`provider_exec_released -> running` and before `initialize`. An empty or
unverifiable identity is `identity_ambiguous`, closes prompt admission, and is
never signalled by PID alone.

`VerifiedProviderLaunchImageV1` is prepared completely before child creation or
secret/environment materialization. The launcher resolves the adapter entrypoint
through a no-symlink dirfd walk, opens every declared launch-closure member with
`O_RDONLY|O_NOFOLLOW`, requires regular files with one link, and records
`(canonical_path_utf8, st_dev, st_ino, st_birthtime_ns, st_size,
executable_sha256)` for each. One closure has at most 4096 files and 512 MiB.
Native Mach-O entrypoints and declared interpreter-plus-script entrypoints are
the only legal kinds; a script never relies on its shebang at execution time.
The runtime copies bytes from those already-open descriptors into a fresh
mode-0700 daemon-owned launch directory, verifies the copied closure and ordered
manifest byte-for-byte, fsyncs files and directory, removes all daemon write
permissions, and retains its directory FD. No provider, workspace, plugin, or
other process receives a writable descriptor or path to that directory.

`VerifiedLaunchDescriptorV1` is the non-cloneable capability over that private
inode set and manifest digest. On macOS, where `fexecve` is unavailable, only
the trusted launch gate consumes it: a native entrypoint is executed from the
private immutable copy, while a script is passed as an argument to the
separately verified private interpreter copy. Immediately before release, both
daemon and gate reopen the declared members through held directory FDs, repeat
no-follow stat/digest verification, and check the directory identity; no
caller-supplied path is accepted. The daemon-wide provider baseline sandbox
denies every provider and descendant write, rename, link, chmod, or unlink
against the private launch root and daemon control roots. Any source/private
path substitution, symlink, hard link, file mutation, manifest drift, oversize,
unsupported entrypoint, injection key, or final verification failure destroys
the private directory and produces zero provider code/instructions and zero
credential resolution. The process binding stores the full launch-image record
before the barrier can release.

Secret-bearing launch data is split at the type boundary. Cloneable,
debuggable, and serializable `ExecutionRequest`, adapter plans, frozen MCP
descriptors, and work-queue envelopes contain only bounded opaque
`CredentialRefV1` values plus non-secret environment-key/header names; raw
environment values, Authorization headers, provider tokens, and Codex auth
bytes are forbidden by compile-time field inventory and serialization sentinel
tests. `ResolvedProviderSecretsV1` is non-`Clone`, has redacted `Debug`, has no
`Serialize`/`Deserialize`, uses locked zeroizing memory, and can be constructed
only by `ProviderSecretResolverV1::resolve(VerifiedLaunchReleasePermitV1)` after
the durable gate-binding readback. Codex runtime-home/auth files are likewise
created only under that permit in a fresh provider-private directory and are
passed to the gate as already-open descriptors; no pre-release path or file is
created.

The release envelope uses `env_clear` and one provider-version-pinned allowlist;
unknown variables and all `DYLD_*`, shell startup, proxy, credential-helper, and
ambient MCP variables are rejected. The launch gate receives resolved values
only over its one-shot pipe, zeroizes its buffer on every branch, closes the
pipe before `execve`, and cannot serialize or log it. Canary credentials cover
every failure before and during gate spawn, binding persistence, helper
verification, private-image verification, release write, target `execve`, and
post-exec identity verification. Every pre-release failure proves resolver call
count zero and no auth file/header/environment materialization; every later
failure proves bounded process cleanup and zeroized private state.

After gate release, `proc_pidpath`, PID/start identity, and the private entrypoint
inode must match the precommitted launch image before `initialize`; mismatch is
`provider_process_identity_unverified` and the same parent reaps the child.
After restart, cleanup compares the persisted private-image manifest and live
process identity before signalling; it never trusts a current installation path.
Source binary replacement, path retarget, parent death, and PID reuse therefore
fail closed without ever executing an unverified source image.

Process cleanup is explicitly parent-aware. Only the still-running recorded
parent daemon on the same boot may use `waitpid` or child-handle reaping. After
daemon restart, cleanup compares the stored boot session, PID, process start
identity, executable identity, and process group against the live process. A
different boot proves absence without signalling; an exact same-boot match may
use verified termination followed by absence observation, but never `waitpid`;
any partial or mismatched identity is `identity_ambiguous` and must not signal
the PID. PID-reuse and parent-death fixtures cover every branch.

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
`agent_execution`, `p017_mediation_execution`, `p079_repair_attempt`,
`p086_continuation`, or `steward_agent_lane`. A database CHECK requires both
execution and occurrence for ordinary/P058/fallback-child `agent_execution`,
requires the owner ID to equal the execution ID with null mediation/P079/
continuation/lane FKs; requires execution and mediation record but null
occurrence/stage for `p017_mediation_execution`, with owner ID equal to the
mediation execution ID; requires parent execution/occurrence plus exact
operation/attempt/lease and null continuation/lane for `p079_repair_attempt`,
with owner ID equal to the lease key; requires execution,
occurrence, and continuation FK for `p086_continuation`, requires owner ID to
equal the continuation ID, and requires a null lane FK; and requires both
execution and occurrence null plus a
non-null lane FK equal to the owner ID for `steward_agent_lane`.
`(configuration_owner_kind, configuration_owner_id, configuration_attempt_index)`
is unique. The owner row stores the next index, nullable active attempt/generation
pair, and current receipt/readiness/failure/cancellation/post-outcome IDs under the
exhaustive compatibility rules above. A permitted zero-send renegotiation appends a new
attempt and atomically moves the applicable pointer; it never overwrites prior evidence.
A configured run-agent insert writes the receipt, pointer, and exact
`agent_executions` projection in one transaction; mismatch on read is evidence
corruption. P017 writes its pointer on the mediation-owned execution without an
occurrence. P079 repair writes its pointer on the lease/attempt and never mutates
the terminal parent execution's status or current configuration pointer. A
configured P086 attachment writes the receipt and its continuation
pointer without modifying the target execution's active configuration attempt
or accepted pair. A configured Codex Steward invocation writes the receipt and
lane pointer because no synthetic `AgentExecution` exists.

`agent_work_continuations` therefore owns non-null
`next_configuration_attempt_index INTEGER NOT NULL DEFAULT 0`, nullable
`active_configuration_attempt_index`, nullable
`active_configuration_generation_id`, nullable
`current_provider_configuration_receipt_id`,
`current_provider_readiness_id`,
`current_provider_configuration_failure_id`,
`current_provider_configuration_cancellation_id`, nullable
`current_provider_post_configuration_outcome_id`, and non-null
`configuration_evidence_state`. P086 attach/reverification reserves and settles
this tuple; it must not borrow the target execution's allocator, active pair, or
receipt pointer. The continuation's requested pair is copied from its frozen
effective contract, while its accepted pair exists only in the continuation-
owned receipt/generation. A target execution and its continuation may therefore
retain different generation-scoped acceptance without either overwriting the
other.

P017 uses the mediation-owned execution row's allocator/pointers under distinct
owner kind `p017_mediation_execution`; its receipt/readiness/failure/
cancellation/post-configuration tables
require null occurrence and stage and the exact mediation record/work item/run
epoch. `output_contract_repair_leases` adds the same allocator, active pair, and
all five current evidence pointers for owner kind `p079_repair_attempt`. P079 repair
cancellation terminalizes only the repair work item, lease, and operation
attempt; the already-terminal parent AgentExecution remains immutable. Generated
owner predicates and direct-SQL fixtures reject substituting an ordinary
execution owner for either special owner.

`GenerationReservationWriterV1` is the sole low-level constructor for a new
configuration attempt/generation/process/binding tuple. It is a private
transaction fragment, not an independently callable operation, and requires a
sealed permit naming the enclosing registered Class A operation. Exactly five
operation families may invoke it:

| Enclosing operation | Legal owner/mode |
|---|---|
| `provider_configuration.reserve_new_generation` | ordinary/P017/P058 initial or frozen-policy recovery; Steward system/auditor turn `0` after sealed claim/activation; Steward turn `1` after its durable retry CAS |
| `p079_repair.admit_or_retry` | lease-bound contained attached generation after supported attach/Seatbelt capability proof; same transaction creates item/turn/generation/process/binding |
| `p079_fallback.admit_or_retry` | lease-bound contained fresh fallback generation or its one authorized zero-send infrastructure retry; same transaction creates child/item/turn/generation/process/binding |
| `p086_continuation.admit` | resurrection admission only |
| `p086_continuation.convert_output_only_to_resurrection` | one-way output-only conversion only |

No operation nests or subsequently calls
`provider_configuration.reserve_new_generation`; the P086 operations invoke
the private fragment inside their own atomic transaction. Live-handle and
existing-generation reuse never invoke it. The fragment's owner-row CAS
pre-generates generation ID `g`, requires both active fields null, reads
`next_configuration_attempt_index = n`, inserts generation `g` in pre-session
state with the same lineage/owner/attempt, and writes next index `n + 1` plus
active pair `(n, g)`. The rebuilt generation table permits null provider-session
and process fields only in this pre-session state. A second caller receives the
enclosing operation's typed conflict and performs zero broker/process/provider
I/O; it does not skip to `n + 1`. The generation, launch intent, eventual
process binding, and logical generation-owner binding all carry `(owner, n, g)`.
Receipt/readiness, failure, or pre-acceptance cancellation settlement requires
that exact active pair. Exact success inserts `(owner, n)`, moves the current
receipt pointer, and clears the readiness/failure/cancellation pointers;
provider-neutral/legacy success binds the unique physical readiness to
`(owner, n, turn)`, moves only the current-readiness pointer, and keeps every
receipt/accepted-pair field null. Either success clears the active pair in its
registered settlement transaction. Failure or cancellation appends only its
typed evidence row, moves only the matching owner pointer, clears the other
class-incompatible pointers, and clears the active pair only after
identity-safe cleanup is terminal; ambiguous cleanup leaves the pair and owner
quarantined for startup. Gaps from a transaction that committed an allocation
but crashed before launch remain valid and are never reused.

The sealed permit for Steward turn `0` requires lane state `reserved`, consumed
counter `0`, active turn exactly `0`, no prior turn, and null active
configuration fields. The turn-`1` permit requires the committed zero-send retry
CAS and immutable failed turn `0`. Claim/activation, generation reservation,
executor load, and prompt dispatch are therefore four ordered authorities; only
the reservation operation can bridge the first two. A race/restart corpus pauses
after claim/activation and before/after reservation commit and proves one
generation/process tuple and zero broker/provider I/O for every loser.

Existing-generation reuse never enters the physical generation-creation path.
It uses the separate
`provider_configuration.reserve_existing_generation` transaction defined above,
which does not create a generation but does atomically consume the configuration
owner's next attempt index while inserting the unique admitted turn/generation
binding with no success evidence and no current-evidence pointer change. A
committed replay returns the stored admitted attempt; the separate class-specific
`settle_success` or `settle_readiness` operation installs the exact receipt or
owner-bound readiness pointer before dispatch. A validation/conflict loser rolls
back without consuming an index.
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
| `provider_configuration.settle_readiness` | `critical_barrier`; owner kind/ID + attempt index + turn ID + generation ID + readiness digest | admit terminal settlement |
| `provider_configuration.settle_failure` | `critical_barrier`; owner kind/ID + attempt index + failure code | admit terminal settlement |
| `provider_configuration.settle_cancellation` | `critical_barrier`; owner kind/ID + attempt index + cancellation code | admit terminal settlement |
| `provider_configuration.settle_post_configuration_outcome` | `critical_barrier`; owner kind/ID + attempt index + class-tagged receipt/readiness digest + typed post-outcome code | admit terminal settlement |
| `provider_configuration.invalidate` | `critical_barrier`; generation + option-snapshot revision/digest | admit safety settlement |
| `provider_prompt_turn.prepare` | `critical_barrier`; prompt-turn ID + owner predicate digest | deny new admission |
| `provider_prompt_turn.settle_sent` | `critical_barrier`; prompt-turn ID + transport outcome digest | admit terminal settlement |
| `provider_prompt_turn.settle_unknown` | `critical_barrier`; prompt-turn ID + quarantine reason | admit terminal settlement |
| `provider_generation_owner.settle` | `critical_barrier`; generation + owner + turn | admit terminal settlement |
| `p079_repair.admit_or_retry` | `critical_barrier`; operation ID + attempt index | deny new admission |
| `p079_fallback.admit_or_retry` | `critical_barrier`; operation ID + attempt index + child execution ID + generation ID | deny new admission |
| `p079_repair.settle_validation` | `critical_barrier`; operation ID + attempt index + validation-evidence digest | admit terminal settlement |
| `p079_repair.complete_no_candidate` | `critical_barrier`; operation ID + attempt index + closed no-candidate reason + validation/absence-witness digest | admit terminal settlement |
| `p079_repair.commit_history_member` | `critical_barrier`; artifact-set ID + member ordinal + staging/history digest | admit terminal settlement |
| `p079_repair.commit_destination_member` | `critical_barrier`; artifact-set ID + member ordinal + history/canonical/expected-activation digest | admit terminal settlement |
| `p079_repair.settle_activation_conflict` | `critical_barrier`; operation/set/member + expected/winning activation tuple + restored-canonical digest | admit terminal settlement |
| `p079_repair.complete_artifact_settlement` | `critical_barrier`; artifact-set ID + ordered history/canonical/expected-activation digest | admit terminal settlement |
| `p086_continuation.admit` | `operator_command` for operator requests, otherwise `critical_barrier`; command journal ID | deny new admission |
| `p086_continuation.convert_output_only_to_resurrection` | `critical_barrier`; continuation + source turn/side-effect/generation/binding + closed cause + frozen context digest | deny new admission |
| `runtime_timeline.persist_event` | `critical_barrier`; run ID + runtime event ID + complete envelope digest | admit terminal settlement |
| `runtime_timeline.ensure_empty_anchor` | `critical_barrier`; run ID + upper sequence + snapshot digest | admit terminal settlement |
| `runtime_timeline.prune_empty_anchors` | `critical_barrier`; expiry bound + ordered anchor-set digest | admit bounded maintenance settlement |
| `steward_analysis.claim` | `critical_barrier`; analysis ID + claimed work-item ID | deny new admission |
| `steward_auditor_lane.activate` | `critical_barrier`; analysis/auditor lane + health-report digest | deny new admission |
| `steward_lane.retry_zero_send` | `critical_barrier`; analysis/lane + retry index 1 | deny new admission |
| `steward_lane.settle` | `critical_barrier`; analysis/lane + terminal digest | admit terminal settlement |
| `runtime_mutation_fence.persist_fatal` | `safety_fence`; next epoch + fatal reason digest | admit safety settlement only |

Migration 100 also creates private
`class_a_operation_results_v1`, owned only by
`db::repos::write_operation_results`:

```sql
CREATE TABLE class_a_operation_results_v1 (
  result_sequence INTEGER PRIMARY KEY CHECK (result_sequence >= 1),
  operation_name TEXT NOT NULL,
  journal_key TEXT NOT NULL,
  request_schema_version TEXT NOT NULL,
  request_sha256 TEXT NOT NULL CHECK (length(request_sha256) = 64),
  result_schema_version TEXT NOT NULL,
  result_json TEXT NOT NULL CHECK (length(result_json) <= 16384),
  result_sha256 TEXT NOT NULL CHECK (length(result_sha256) = 64),
  membership_count INTEGER NOT NULL CHECK (membership_count >= 0),
  membership_sha256 TEXT NOT NULL CHECK (length(membership_sha256) = 64),
  previous_result_chain_sha256 TEXT NOT NULL CHECK (length(previous_result_chain_sha256) = 64),
  result_chain_sha256 TEXT NOT NULL CHECK (length(result_chain_sha256) = 64),
  committed_at TEXT NOT NULL,
  UNIQUE (operation_name, journal_key),
  UNIQUE (result_sequence, result_chain_sha256)
);

CREATE TABLE class_a_operation_result_members_v1 (
  operation_name TEXT NOT NULL,
  journal_key TEXT NOT NULL,
  member_ordinal INTEGER NOT NULL CHECK (member_ordinal >= 0),
  member_kind TEXT NOT NULL,
  natural_owner_id TEXT NOT NULL CHECK (length(natural_owner_id) <= 512),
  member_sha256 TEXT NOT NULL CHECK (length(member_sha256) = 64),
  PRIMARY KEY (operation_name, journal_key, member_ordinal),
  UNIQUE (operation_name, journal_key, member_kind, natural_owner_id),
  FOREIGN KEY (operation_name, journal_key)
    REFERENCES class_a_operation_results_v1(operation_name, journal_key)
    DEFERRABLE INITIALLY DEFERRED
);

CREATE TRIGGER class_a_result_immutable_update
BEFORE UPDATE ON class_a_operation_results_v1
BEGIN SELECT RAISE(ABORT, 'class_a_result_immutable'); END;

CREATE TRIGGER class_a_result_immutable_delete
BEFORE DELETE ON class_a_operation_results_v1
BEGIN SELECT RAISE(ABORT, 'class_a_result_immutable'); END;

CREATE TRIGGER class_a_result_member_sealed_insert
BEFORE INSERT ON class_a_operation_result_members_v1
WHEN EXISTS (
  SELECT 1 FROM class_a_operation_results_v1 p
   WHERE p.operation_name = NEW.operation_name
     AND p.journal_key = NEW.journal_key
)
BEGIN SELECT RAISE(ABORT, 'class_a_result_already_sealed'); END;

CREATE TRIGGER class_a_result_member_immutable_update
BEFORE UPDATE ON class_a_operation_result_members_v1
BEGIN SELECT RAISE(ABORT, 'class_a_result_member_immutable'); END;

CREATE TRIGGER class_a_result_member_immutable_delete
BEFORE DELETE ON class_a_operation_result_members_v1
BEGIN SELECT RAISE(ABORT, 'class_a_result_member_immutable'); END;

CREATE TRIGGER class_a_result_chain_and_membership_guard
BEFORE INSERT ON class_a_operation_results_v1
BEGIN
  SELECT CASE WHEN NEW.result_sequence <> COALESCE(
    (SELECT MAX(result_sequence) + 1 FROM class_a_operation_results_v1), 1
  ) THEN RAISE(ABORT, 'class_a_result_sequence_non_successor') END;
  SELECT CASE WHEN NEW.previous_result_chain_sha256 <> COALESCE(
    (SELECT result_chain_sha256
       FROM class_a_operation_results_v1
      ORDER BY result_sequence DESC LIMIT 1),
    '0000000000000000000000000000000000000000000000000000000000000000'
  ) THEN RAISE(ABORT, 'class_a_result_predecessor_mismatch') END;
  SELECT CASE WHEN NEW.result_chain_sha256 <>
    chainworks_class_a_result_chain_sha256(
      NEW.result_sequence, NEW.previous_result_chain_sha256,
      NEW.operation_name, NEW.journal_key, NEW.request_sha256,
      NEW.result_schema_version, NEW.result_sha256,
      NEW.membership_count, NEW.membership_sha256
    ) THEN RAISE(ABORT, 'class_a_result_chain_digest_mismatch') END;
  SELECT CASE WHEN NEW.membership_count <> (
    SELECT COUNT(*) FROM class_a_operation_result_members_v1 m
     WHERE m.operation_name = NEW.operation_name
       AND m.journal_key = NEW.journal_key
  ) THEN RAISE(ABORT, 'class_a_result_membership_count_mismatch') END;
  SELECT CASE WHEN EXISTS (
    SELECT 1 FROM class_a_operation_result_members_v1 m
     WHERE m.operation_name = NEW.operation_name
       AND m.journal_key = NEW.journal_key
       AND m.member_ordinal <> (
         SELECT COUNT(*) FROM class_a_operation_result_members_v1 p
          WHERE p.operation_name = m.operation_name
            AND p.journal_key = m.journal_key
            AND p.member_ordinal < m.member_ordinal
       )
  ) THEN RAISE(ABORT, 'class_a_result_membership_ordinal_gap') END;
  SELECT CASE WHEN NEW.membership_sha256 <> COALESCE((
    SELECT chainworks_membership_sha256(
      member_kind, natural_owner_id, member_ordinal
    ) FROM (
      SELECT member_kind, natural_owner_id, member_ordinal
        FROM class_a_operation_result_members_v1
       WHERE operation_name = NEW.operation_name
         AND journal_key = NEW.journal_key
       ORDER BY member_ordinal
    )
  ), chainworks_empty_membership_sha256())
  THEN RAISE(ABORT, 'class_a_result_membership_digest_mismatch') END;
END;
```

`journal_key` is the domain-separated SHA-256 of the exact replay components in
the registry row, encoded with the common length-prefixed codec; raw IDs are not
concatenated. `request_sha256` and `result_sha256` are SHA-256 over
duplicate-key-rejected RFC 8785 JSON with explicit schema/version tags. Result
JSON is private authority evidence, contains no provider-session secret or raw
provider payload, and uses the operation's closed Rust result enum. The
transaction first loads `(operation_name, journal_key)`: matching request digest
runs that registry entry's closed `ClassAReplayVerifierV1` and returns the stored
typed result without domain writes, a different digest returns `Conflict`, and
absence permits the mutation. It inserts the result row in the same transaction
as all effect rows. Decode, schema, digest, or replay-verifier failure is fatal
evidence corruption, never `AlreadyMatching`.

`result_sequence` is the global append order assigned by the single DbWriter.
The first row uses 64 zeroes as `previous_result_chain_sha256`; every later row
must name the immediately preceding row's `result_chain_sha256`.
`result_chain_sha256` is the domain-separated common-codec SHA-256 over
`[result_sequence, previous_result_chain_sha256, operation_name, journal_key,
request_sha256, result_schema_version, result_sha256, membership_count,
membership_sha256]`. The writer reads the predecessor and inserts the successor
under the same transaction and global commit barrier as the effect/result rows.
Triggers reject a missing predecessor, a non-successor sequence, or a chain
digest mismatch. Sequence gaps, forks, and any historical mutation are fatal
evidence corruption.

`chainworks_class_a_result_chain_sha256`,
`chainworks_membership_sha256`, and
`chainworks_empty_membership_sha256` are deterministic functions registered on
every migration, writer, and verification connection before any statement can
touch these tables. Their exact domains, common-codec inputs, and known-answer
vectors are checked in. `Migration100SchemaManifestV1` includes normalized
`sqlite_master` SQL and SHA-256 for both tables, every unique/non-semantic
index, all six immutable/seal/chain triggers above, every foreign key and
deferred flag, and the function-version digest. Fresh and migrated databases
must byte-match that complete manifest before preflight can publish a pool.
Direct-SQL mutation tests insert first-row and successor gaps, duplicate/forked
sequences, wrong predecessor, wrong chain digest, missing/extra/reordered
members, wrong count/digest, and parent-first or post-seal children; every case
aborts both on a fresh migration and after migration-100 finalization.

Result JSON contains only fixed-size outcome fields and a membership summary;
it never embeds an unbounded collateral-owner array. For any operation with a
variable owner set, the transaction canonical-sorts `(member_kind,
natural_owner_id)`, inserts one child membership row per owner, and stores count
plus SHA-256 over the common-codec ordered member tuples in the parent result.
The registered deterministic SQLite aggregate
`chainworks_membership_sha256(kind, id, ordinal)` is available on every writer
connection; the parent insert trigger requires exact contiguous ordinals,
count, and aggregate digest before commit. Empty membership uses count zero and
the frozen empty-list digest. Replay streams child rows in ordinal order,
recomputes count/digest, and then verifies every natural owner row. A missing,
duplicate, reordered, truncated, or extra member is fatal evidence corruption.
The 16 KiB bound therefore applies only to the fixed parent result, not owner
cardinality; a 512-owner settlement and restart replay are retained fixtures.
Members are always inserted before the parent in the same deferred-FK
transaction; parent insertion is the irreversible seal. Once the parent exists,
the late-insert trigger rejects every additional child, including for an empty
result. Direct-SQL fixtures cover parent/member update and delete, post-seal
insert, wrong-order parent-first insertion, and exact sealed results with
0, 1, and 512 members across restart.

Class A results witness the effect at their own linearization point; they do not
assert that every mutable owner projection remains byte-equal forever. Each
registry row declares exactly one generated replay rule:

- `exact_immutable` requires the listed append-only/immutable rows and digests
  to remain byte-equal;
- `closed_successor(<graph>)` requires the immutable identity/correlation/effect
  rows byte-equal and permits each mutable row only in a named monotonic
  successor of the committed result. The graph is generated from the same
  reducer enum used for writes; changing an immutable field, regressing state,
  skipping required evidence, or reaching an unlisted successor is corruption.

Allocator counters may be greater than the committed lower bound, current
pointers may name a later attempt, and the fatal singleton may name a later
cycle/open epoch only when the original immutable attempt/receipt/failure/turn/
cycle witness still exists. There is no generic "current row equals result"
fallback. The exhaustive result/replay mapping is:

| Registered operation | Result codec | Effect rows and replay rule |
|---|---|---|
| new-generation configuration reservation | `NewGenerationReservationResultV1` | immutable attempt/generation/process/binding with null receipt; owner allocator lower bound and active pointer use `closed_successor(configuration_owner_v1)` |
| existing-generation configuration reservation | `ExistingGenerationReservationResultV1` | immutable admitted attempt/binding with null success evidence; owner allocator uses `closed_successor(configuration_owner_v1)`, and class-specific settlement is a separate result |
| exact configuration success/failure/cancellation | `ProviderConfigurationSettlementResultV1` | immutable receipt, failure, or cancellation; generation and owner pointer use `closed_successor(configuration_settlement_v1)` |
| provider-neutral/legacy readiness | `ProviderReadinessSettlementResultV1` | immutable physical readiness plus exact logical owner/attempt/turn binding; owner pointer and binding use `closed_successor(configuration_settlement_v1)` |
| post-readiness zero-prompt outcome | `ProviderPostConfigurationOutcomeSettlementResultV1` | immutable class-appropriate receipt/readiness plus post-outcome row; turn remains `not_started`; binding/owner pointers use `closed_successor(configuration_settlement_v1)` and no failure/cancellation/runtime receipt may appear |
| configuration invalidation | `ProviderConfigurationInvalidationResultV1` | exact append-only invalidation; generation/affected owners use `closed_successor(configuration_invalidation_v1)` and can never become dispatch-valid again |
| prompt prepare/sent/unknown | `PromptTurnCasResultV1` | immutable turn identity and dispatch timestamps/evidence; state uses `closed_successor(prompt_turn_v1)` |
| generation-owner settle | `GenerationOwnerSettlementResultV1` plus membership count/digest | exact terminal binding/result and immutable membership; owner projections use `closed_successor(owner_terminal_v1)` |
| P079 admit/retry | `P079AttemptAdmissionResultV1` | exact slot/link/identity; operation, lease, item, turn, and child use `closed_successor(p079_attempt_v1)` |
| P079 fallback admit/retry | `P079FallbackAdmissionResultV1` | exact slot/lease/child/item/turn/generation/process/binding tuple; owner rows use `closed_successor(p079_fallback_attempt_v1)` |
| P079 validation prepare | `P079PostValidationSettlementResultV1` | exact validation/artifact-set/member witness; operation/set/members/parent use `closed_successor(p079_artifact_set_v1)` |
| P079 no-candidate completion | `P079NoCandidateCompletionResultV1` | exact closed reason, validation/absence witness, zero-member completed set, terminal repair event, item/lease/operation result, and unchanged parent hold are `exact_immutable` or named closed successors; no filesystem or activation row may exist |
| P079 history member commit | `P079HistoryMemberCommitResultV1` | exact staging/history digest and durable member/set history state; immutable history witness plus closed successor state |
| P079 destination member commit | `P079DestinationMemberCommitResultV1` | exact quarantine or canonical/activation result and durable member/set destination state; immutable activation history plus current-pointer closed successor |
| P079 activation conflict settle | `P079ActivationConflictSettlementResultV1` | exact losing expected revision, winning activation/current bytes, canonical restore, conflict row, conflict-settled member/set, terminal operation/event, and unchanged parent hold are immutable or named closed successors |
| P079 artifact completion | `P079ArtifactCompletionResultV1` | completed set/member/history/activation evidence is `exact_immutable`; current activation and parent projections use named closed-successor graphs |
| P086 admit | `P086ContinuationAdmissionResultV1` | exact command and item/turn identities plus mode-dependent nullable context/window/monotonic-clock/attempt/generation/process/binding tuple; resurrection requires the complete tuple, live/output-only initial admission requires it absent; lifecycle rows use `closed_successor(p086_continuation_v1)` |
| P086 output-only conversion | `P086ResurrectionConversionResultV1` | exact source turn/side-effect/generation/binding, immutable cause, replacement turn/side-effect, context/window/monotonic-clock/attempt/generation/process/binding identities; lifecycle rows use `closed_successor(p086_continuation_v1)` and source rows require the named zero-send terminal successor |
| Timeline event persist | `RuntimeTimelineEventPersistResultV1` | exact immutable event/lane/cursor row; global and lane allocator lower bounds use `closed_successor(runtime_timeline_v1)` and publication occurs only after this result commits |
| Timeline empty anchor | `RuntimeTimelineEmptyAnchorResultV1` | exact immutable run/upper-sequence/snapshot/cursor anchor is `exact_immutable`; same-key replay returns the same cursor and no event row |
| Timeline empty-anchor prune | `RuntimeTimelineAnchorPruneResultV1` | exact expired anchor/lease membership and delete tombstone digest are immutable; unexpired or unlisted rows are forbidden |
| Steward analysis claim | `StewardAnalysisClaimResultV1` | exact analysis/lane/turn identities; lifecycle rows use `closed_successor(steward_analysis_v1)` |
| Steward auditor activation | `StewardAuditorActivationResultV1` | exact prerequisite/turn identity; lane uses `closed_successor(steward_lane_v1)` |
| Steward zero-send retry | `StewardLaneRetryResultV1` | exact terminal turn-0 evidence and turn-1 identity; lane uses `closed_successor(steward_lane_v1)` |
| Steward settle | `StewardLaneSettlementResultV1` | terminal artifacts are exact; lanes/analysis/item use `closed_successor(steward_terminal_v1)` |
| fatal mutation fence | `RuntimeMutationFenceResultV1` | fatal-cycle/result are `exact_immutable`; singleton uses `closed_successor(fatal_cycle_v1)` and requires reconciliation before a later open epoch |

Every codec contains its natural IDs and closed outcome; no generic string
result is accepted. The registry generator requires exactly one mapping row and
codec implementation for every operation name. Commit-before-ack reconciliation
reads this journal, runs the listed replay verifier in one read transaction,
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

Class A submission is bounded before any transaction can start.
`ClassAOperationPermitV1` has fixed process-wide capacity 512, partitioned into
384 admission/reservation permits, 127 terminal/cleanup permits, and one
non-borrowable fatal-fence permit. Terminal work may borrow an idle admission
permit, but admission may never consume the terminal/fatal reserve. A permit is
held from queue acceptance through known acknowledgement or is moved, never
copied, into reconciliation on `Unknown`. Therefore the number of unresolved
envelopes and pollers cannot exceed 512. Admission overload is
`rejected_before_start` with no transaction. Terminal overload first closes new
admission and enters the priority queue ahead of all admission work; if its
bounded wait expires before start, it invokes the dedicated fatal path and
leaves the domain owner for startup reconciliation. The fatal permit cannot be
starved by either class.

`ClassAReconciliationRegistryV1` is keyed by `(operation_name, journal_key)`.
Handoff for an existing key coalesces onto the same immutable request digest,
task, permit, and result future; a different digest is immediate `Conflict`.
There is exactly one daemon-owned supervisor task per distinct key, not one per
caller. The task is uncancellable by the originating request and is owned by
the daemon supervisor until process exit. It retains the writer task/journal
key, waits for writer completion when available, and polls the result journal
with bounded exponential backoff. An exact result runs the registered replay
verifier over immutable witnesses plus the closed successor graph and then runs
the operation's idempotent completion callback; proven transaction
rollback/no-start runs the typed failure callback. It never resubmits the
mutation or permits provider/process/prompt I/O. If neither result nor rollback
can be proved within 10 seconds measured by the current boot's monotonic
continuous clock, `close_first_fatal` transfers
ownership to restart; the process does not continue normal service.

P086 operation envelopes additionally carry their immutable resurrection-window
ID and setup-cleanup deadline. While the turn is not `prompt_sent`, their same-
process supervisor may continue only DB journal/replay verification after the
setup deadline; every completion callback
is forced into cleanup-only reduction and cannot return broker, process-launch,
attach, configuration, or prompt authority. Its reconciliation deadline is
`min(supervisor_start + 10 seconds, setup_cleanup_deadline_continuous_ns)`. At that
instant it atomically marks the already-durable pending envelope for startup,
calls `close_first_fatal`, and terminates the same-process task. Startup resumes
the exact key under `P086ExpiredWindowReconcilerV1`; no detached setup operation
outlives the setup-cleanup deadline. A result discovered after the setup
deadline but before setup-cleanup expiry may only prove or settle zero-send or
delivery-unknown evidence for the existing window. Once the canonical turn is
`prompt_sent`, this setup envelope is terminal and the ordinary execution-
watchdog/terminal-settlement supervisor owns later response and cleanup work.

Timeout-storm fixtures submit 10,000 duplicate and distinct requests while
pausing writer acknowledgements. They assert at most 512 unresolved keys/tasks,
same-key coalescing, no transaction for rejected admission, terminal work
ordered ahead of queued admission, availability of the fatal permit, and exact
permit release after known result, proven rollback, or process-exit transfer to
startup.

Migration 100 also creates durable `class_a_reconciliation_pending_v1` intents
for every Class A operation and singleton
`class_a_reconciliation_checkpoint_v1(last_result_sequence,
last_result_chain_sha256, updated_at)`, initialized to sequence `0` and the
64-zero chain sentinel. Pending rows store at most the 16 KiB
canonical request envelope, operation/key/digest, owner selector, transfer boot
ID/time, and one-way `pending -> resolved_result | proved_rollback_or_no_start |
fatal` state. The small idempotent intent insert is acknowledged before the
domain transaction can start and before any provider/process/prompt I/O; an
uncertain intent acknowledgement is read back by exact key, and failure to prove
it closes first fatal without starting the operation. The domain transaction
that inserts effect/result rows also moves its intent to `resolved_result`.
Process death rolls an uncommitted domain transaction back while leaving the
intent, so startup can prove rollback/no-start from the absent result and absent
registered natural witnesses before terminalizing the intent. Startup consumes
both sources: unresolved pending intents first, then result rows strictly after
the durable high-water sequence.

Restart reconciliation never scans the complete historical journal. Before
consumers open it verifies and replays batches capped at both 256 parent results
and 4 MiB of parent/member/envelope bytes. Each batch starts from the exact
checkpoint chain digest, verifies contiguous sequence/hash links and registered
natural-owner selectors, runs idempotent completion, and advances the checkpoint
in one transaction only after the whole batch is proven. A fresh startup has an
exact 15-second `mach_continuous_time` budget. Budget expiry preserves the last
completed checkpoint, keeps the daemon failed-serve, and the next restart
continues at `last_result_sequence + 1`; it never restarts from sequence 1 or
skips unresolved pending envelopes. Same-boot wall jumps do not affect the
budget, and a reboot simply begins a new 15-second startup budget without
changing the durable high-water.

A late commit is therefore observed either by the same-process supervisor or by
bounded startup, while a process death aborts an uncommitted SQLite transaction.
The generated registry provides one reconciliation callback and one startup
selector per result codec; missing either fails the gate. Faults delay commit
until after the immediate read, drop the caller and acknowledgement, stop the
supervisor before/after journal visibility, and restart. Every case converges to
one stored result with zero duplicate I/O. A retained one-million-result fixture
plants unresolved envelopes before, at, and after multiple checkpoints and
proves constant-memory batches, no read at or below the high-water, exact chain
continuity, progress across forced 15-second budget exits, and no duplicate
domain/provider I/O. A post-I/O unknown terminal write invokes
`close_first_fatal` immediately rather than waiting for the ordinary deadline.

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
current_fatal_cycle_id, updated_at)`, where state is `open|fatal`, plus immutable
`runtime_fatal_cycles_v1(cycle_id, process_boot_id, open_epoch, fatal_epoch,
fatal_reason_sha256, persisted_at)` and append-only
`runtime_fatal_cycle_reconciliations_v1(cycle_id, reopened_epoch,
reconciled_at)`. Cycle ID is common-codec identity over process boot ID and
fatal epoch; cycle rows reject update/delete and one cycle has at most one
reconciliation row. The db crate owns the matching in-memory epoch plus one
commit-barrier mutex.

The global runtime writer order is exactly `commit barrier -> BEGIN IMMEDIATE ->
domain/result writes -> COMMIT/ROLLBACK -> release barrier`. Each queued mutation
captures an open epoch before waiting, acquires the barrier before any SQLite
write transaction or write lock, rechecks that epoch and open state, and retains
the barrier through commit or rollback. No runtime path may acquire a SQLite
write lock and then wait for the barrier. Read-only preparation happens before
the barrier and is revalidated inside the transaction. Startup/preflight writes
run before runtime publication under `PreflightLockGuard` and never race this
order.

Daemon owns exactly one `FirstFatalCoordinator`; no crate receives separate
mutation-fence or prompt-fence close authority. Its sole mutation method
`close_first_fatal(FatalServeReasonV1)` acquires the commit barrier and a
first-reason latch, ignores a later reason, increments the in-memory epoch,
closes both `RuntimeMutationFenceV1` and `PromptAdmissionFence`, and invokes the
private shutdown-proof `runtime_mutation_fence.persist_fatal` transaction on a
dedicated connection while still owning fatal linearization. Because every
ordinary writer releases its SQLite transaction before the coordinator can own
the barrier, this fatal-only transaction cannot invert lock order. It inserts
the immutable cycle/result rows and CAS-persists the singleton's exact next
epoch/reason/cycle pointer; it is the sole write admitted after closure. Only
after both rows and the Class A result are readable does the coordinator disable the ordinary writer queue and
publish the failed-serve watch notification. Therefore a commit holding the
barrier linearizes before fatal; `close_first_fatal` holding it first forces
every old-epoch transaction to roll back, and prompt admission closes at the
same point. There is no second compare-exchange linearization point.

If fatal persistence returns failure or `Unknown`, both in-memory fences and the
first-reason latch remain closed/frozen for the rest of that process, no watch
success is claimed, and daemon exits with the fatal bootstrap code. The
proposal does not claim a durable fatal cycle that SQLite did not commit. On the
next process, guarded preflight first reconciles the fatal operation key: if its
cycle/result committed, it follows the normal immutable-cycle path; if complete
transaction rollback/no-start is proved, it performs the exhaustive Class A,
prompt-turn, owner, process, and SQLite-integrity reconciliation and may reopen
from the last durable epoch only when every selector is terminal and no process
identity remains unresolved. Any unknown or inconsistent row keeps
`preflight_failed`. Thus persistence loss is fail-closed for the current process
but does not create an unverifiable permanent cross-restart prohibition.

Clean startup reconciliation of a committed fatal verifies the exact fatal-
cycle result by immutable cycle ID rather than expecting the singleton to remain
at that old epoch, inserts its one reconciliation row, advances the singleton to
a new open epoch with null cycle/reason, and only then produces
`PreflightCompleteToken`; no running process reopens it. A later process may
therefore create a distinct fatal cycle without invalidating replay of the
first. Concurrency fixtures race every fatal source and a writer paused before
barrier/BEGIN/commit, require one immutable reason per cycle, prove
persist-before-notify ordering, and run two complete fatal/restart/reopen cycles
without result mismatch or deadlock.

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

Reservation predicates are mode-specific; there is no global requirement that
all historical owner turns remain `not_started`:

| Reservation mode | Required turn/generation truth |
|---|---|
| initial/new generation | target turn is `not_started`; no other active owner binding; any prior generation is identity-safely closed and every prior turn is terminal |
| frozen-policy new generation after zero-send failure | target replacement turn is `not_started`; predecessor is terminal zero-send evidence; policy-specific retry CAS is committed |
| existing-generation specialized turn | only the new target turn is `not_started`; historical `prompt_sent` turns and their receipts remain immutable; the physical generation is live, identity-matched, and has no other active binding |
| contained attached P079 repair generation | target repair turn is `not_started`; source turn/receipt are immutable and terminal; source provider-session ref and supported attach protocol are exact; a new sandboxed physical generation/process is precommitted |

The new-generation modes atomically mark a prior dispatch pointer superseded for
future dispatch before installing `(n, g)`; they never rewrite historical
turn-to-receipt links. Existing-generation P086 reservation and contained-attached
P079 reservation each derive a new owner attempt/receipt for the target turn and
advance only that specialized owner's current pointer while preserving original
sent turn A, receipt A, and the original AgentExecution current pointer as
historical parent evidence. P079 uses physical generation B attached to the same
private provider-session identity; it never reuses broad process/generation A. A failed
new attempt can never reactivate old acceptance. Every mode requires active
fields null at its CAS, so it cannot race an in-progress reservation. The
retained A/B fixture sends original turn A, reserves contained repair turn B on
physical generation B, and proves both receipts plus provider-session
correlation without a fresh logical session.

The dispatch CAS consumes the required tagged union
`ProviderDispatchEvidenceV1`: `exact_receipt(receipt_id, receipt_sha256)` is
legal only for `codex_exact_pair`, while
`provider_readiness(readiness_id, readiness_sha256)` is legal only for
`provider_neutral | legacy_best_effort`. It joins that evidence through the
owner binding to the exact attempt, generation, work item, and prompt turn, then
joins the generation to one running, identity-verified process binding. The
resulting private permit carries the same class tag and is bound to owner,
attempt, evidence ID/digest, generation, provider session, process-binding
ID/start identity, effective contract, work item, and prompt turn. A receipt
cannot satisfy a readiness class, readiness cannot satisfy exact Codex, and a
stale pointer, different generation/process, or another attempt/turn is not
transferable authority.

Historical rows backfill next index `0`, both active fields null, and a null
pointer; rows for which migration creates a receipt use one greater than the
greatest inserted attempt and point at that row. The final schema rebuild
installs owner active-generation and all receipt/readiness/failure/cancellation/
post-outcome FKs after every target table exists.
Tests race two allocators for each owner kind, crash every reservation/launch/
settlement boundary, and prove exactly one launched generation, monotonic gaps,
pointer CAS, stale-attempt rejection, and one dispatch-capable receipt.

`provider_configuration_failures` is append-only with primary key `id` and a
unique key on owner kind/ID plus attempt index. It stores non-null owner kind/ID,
configuration-attempt index, `prompt_turn_id`, and work item; nullable
generation/process binding; typed non-cancellation failure code; optional
source-acceptance digest; immutable `cleanup_required`; and timestamps, but no
accepted pair or provider-session secret. `provider_configuration_cancellations`
is a separate append-only table with the same owner/attempt/turn/work-item and
nullable generation/process identity, exact code
`cancelled_before_configuration`, immutable `cleanup_required`, and
`cancelled_at`; it likewise stores no accepted pair or provider-session secret.
Both tables reject update/delete and are mutually exclusive with each other and
with a receipt for the same owner/attempt.

Cleanup progress never mutates either evidence row. Append-only
`provider_configuration_cleanup_events_v1` stores evidence kind/ID, contiguous
event ordinal, exact generation/process identity, and closed event
`cleanup_required | never_launched | reaped | identity_ambiguous`, with a unique
terminal event per evidence row. Its generated read-only projection reduces
the greatest contiguous ordinal to `pending | terminal_absent | terminal_reaped
| terminal_ambiguous`; direct update/delete and an event after terminal are
rejected. The failure/cancellation/post-readiness settlement transaction appends
ordinal 0 when cleanup is required, and the identity-safe cleanup transaction
appends exactly one terminal event. Thus a crash can leave durable pending
cleanup without rewriting append-only failure, cancellation, or outcome truth.

`provider_post_configuration_outcomes_v1` is separate append-only evidence for
class-appropriate readiness whose prompt turn remains `not_started`. It stores
exact owner/attempt/generation/turn/work-item identity, provider-contract class,
nullable receipt ID, nullable provider-readiness ID, closed code, immutable
`cleanup_required`, and observed time. Exact Codex requires only the receipt;
provider-neutral/legacy requires only readiness. The closed codes are
`configured_cancelled_before_prompt | configured_deadline_before_prompt |
configured_transport_lost_before_dispatch |
configured_superseded_for_resurrection |
provider_ready_cancelled_before_prompt |
provider_ready_deadline_before_prompt |
provider_ready_transport_lost_before_dispatch |
provider_ready_superseded_for_resurrection`. The two supersession codes are
legal only for the one-way P086 output-only conversion and select the receipt
or readiness variant respectively; every other cause likewise selects its
class-matching `configured_*` or `provider_ready_*` code. The row preserves its receipt/readiness,
cannot coexist with a configuration failure/cancellation for that attempt, and
is never an `AcpRuntimeReceipt`. Update/delete are rejected. The binding's
nullable `post_configuration_outcome_id` may coexist only with its
class-appropriate receipt/readiness in terminal or cancelled state.

`ProviderConfigurationFailureCodeV1` is the closed enum
`model_unavailable | model_not_accepted | effort_unavailable |
effort_not_accepted | acceptance_persistence_failed | provider_start_failed |
provider_process_identity_unverified | configuration_deadline_elapsed |
resume_unsupported | resume_configuration_unavailable |
configuration_evidence_invalid`. `ProviderConfigurationCancellationCodeV1` has
the sole value `cancelled_before_configuration`.
`ProviderPostConfigurationOutcomeCodeV1` has exactly the eight values above.
`ProviderPromptTurnFailureCodeV1` is the separate closed prompt-lifecycle enum
`configuration_failed | prompt_preparation_failed |
owner_cancelled_before_prompt | owner_superseded_before_prompt |
provider_generation_interrupted_before_prompt | prompt_transport_failed |
prompt_delivery_unknown | provider_runtime_failed_after_prompt |
provider_runtime_timeout_after_prompt |
provider_generation_interrupted_after_prompt | legacy_authority_unverifiable`.
Rust/domain/DB/GraphQL/Swift use these enums directly; no free-form string or
provider error text can populate a code field. Only the configuration-row ACP
constants in the failure-behavior table map one-to-one to these lower-snake-case
durable values in a generated table; owner validation, dispatch, quarantine, and
fatal-admission failures remain their separate closed domain outcomes.

Negotiation/startup failure inserts only the failure row, sets
`failed_before_prompt`, sets evidence `failure_available`, points the owner at
that row, clears receipt/cancellation pointers, and keeps the turn
`not_started`. Pre-acceptance cancellation inserts only the cancellation row,
sets `cancelled_before_configuration`, sets evidence `cancellation_available`, points
the owner at that row, clears receipt/failure pointers, and keeps the turn
`not_started`. Cancellation after accepted configuration retains the receipt,
accepted pair, and `receipt_available` evidence, sets
`configured_terminated_before_prompt`, and appends
`configured_cancelled_before_prompt`; it creates no cancellation/failure row.
P086 output-only conversion similarly preserves the source class evidence and
appends exactly one `configured_superseded_for_resurrection` or
`provider_ready_superseded_for_resurrection` outcome in the same Class A
transaction that terminalizes the source binding and creates the replacement
turn. A free-form prompt-turn failure string named
`superseded_for_resurrection` is not durable settlement authority.
If receipt persistence fails after provider acceptance, the receipt transaction
rolls back, the manager sends zero prompt, identity-safely closes the generation,
and writes the ordinary failure row with durable code
`acceptance_persistence_failed`; no receipt is created. If minimal
failure/cancellation settlement cannot commit, daemon enters failed-serve;
startup classifies/reaps the still-configuring generation and writes exactly one
matching failure or cancellation row before consumers open. No path invents a
`ProviderConfigurationReceiptV1` or `AcpRuntimeReceipt` for failure/cancellation.

`provider_prompt_turns` has `id` as primary key; non-null `prompt_kind`,
`turn_index`, `prompt_owner_kind`, `prompt_owner_id`, `work_item_id`, `provider`,
and `transport_family`; nullable generation/session IDs, run ID, stage execution
ID, agent ID, agent execution, occurrence, captured run epoch, `mediation_record_id` FK,
`escalation_ledger_id` FK, and `steward_lane_id` FK; contract version;
nullable `p079_operation_id`, `p079_attempt_index`, and `p079_lease_key`;
`dispatch_state`;
start/sent/unknown timestamps; nullable
`ProviderPromptTurnFailureCodeV1`; and created/updated
timestamps. Foreign keys bind execution when present and always bind the work
item. Owner kind is `invoke_agent`, `p017_mediation`, `p058_escalation`,
`p079_repair`, `p079_fallback_child`, `p086_continuation`, or
`steward_agent_lane`. The ordinary `invoke_agent`, P058, both P079 kinds, and
P086 branches require execution, occurrence, captured run epoch, run/stage/agent
IDs, and null lane FK. P017 is a separate stage-less branch: it requires
non-null run ID, captured run epoch, agent ID, mediation-owned AgentExecution,
mediation-record FK, and InvokeAgent work item; requires
`prompt_owner_id = agent_execution_id`; and requires stage-execution ID,
task-occurrence ID, escalation FK, lane FK, and all P079 fields null. Insert/
update triggers require the AgentExecution to have
`owner_kind = lead_conflict_mediation`, `stage_execution_id IS NULL`,
`owner_id = lead_mediation_record_id = mediation_record_id`, and require the
work-item payload's run/mediation/execution identity to match. Thus P017 binds
run cancellation through captured run epoch and mediation ownership without
inventing a stage or occurrence.
Prompt-turn failure nullability is exact. An active `not_started`,
`dispatch_pending`, or successful `prompt_sent` turn has null failure code. A
terminal zero-send `not_started` turn uses one of the first six or
`legacy_authority_unverifiable`, with detailed configuration cause retained only
in `configurationTruth`. `dispatch_unknown` requires exactly
`prompt_delivery_unknown`. A terminal failed `prompt_sent` turn requires one of
the three `*_after_prompt` runtime/interruption values. Any other state/code pair
is rejected by SQL, Rust decode, GraphQL, and Swift fixtures; raw provider/error
text is never stored in this column.
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

`PromptTurnAllocator::reserve_tx` is the only low-level constructor, is
crate-private, and accepts a sealed owner-specific allocation permit. Claim/start inserts
`original/0`, sets `next_prompt_turn_index = 1`, and creates the execution in one
transaction for ordinary, P017 mediation-owned, and P058 escalation-owned
AgentExecutions. Ordinary/P058 creation copy-validates and copies the already-
enqueued occurrence ID/sequence and never invokes `TopologyOccurrenceAllocator`.
P017 instead writes the exact stage-less null tuple above and binds mediation
record/run epoch/work item in the same transaction. P058 binds the
escalation ledger. Every later run-bound prompt atomically reads/increments that
counter, so P079 and P086 cannot both claim index 1. A Steward invocation uses
the durable `steward_agent_lanes.id`, reads/increments that lane's
`next_prompt_turn_index`, inserts `steward_analysis/<allocated index>`, and never
allocates from an `AgentExecution`. The exact Steward call sites are closed:
`steward_analysis.claim` alone constructs the system lane and its turn `0`;
`steward_auditor_lane.activate` alone constructs auditor turn `0` after the
validated prerequisite; and `steward_lane.retry_zero_send` alone constructs turn
`1` after its durable zero-send retry CAS. The executor receives and loads an
existing turn ID and has no allocation permit. Generic work claim, startup
requeue, configuration reservation, and prompt execution cannot create a turn.
Exact prompt
kinds are `original`, `code_writer_completion_repair`,
`output_contract_repair`, `work_continuation_live_handle`,
`work_continuation_resurrection`, `work_continuation_output_only`, and
`steward_analysis`; adding a kind requires a migration-safe enum and gate
fixture. A deterministic `prompt_turn_v1:<sha256>` hashes prompt owner kind/ID,
allocated index, kind, a tagged nullable execution/occurrence tuple, and
work-item ID with the canonical length-prefixed codec.

P017 executable fixtures create a canonical stage-less mediation execution and
turn, then cross dispatch success, cancellation before dispatch, cancellation
during `dispatch_pending`, unknown delivery, and restart. Direct-SQL negatives
attempt to add a stage/occurrence, remove or swap mediation identity, or use a
stage-owned execution/work item; all fail before provider I/O.

The existing runtime receipt primary key on agent execution, prompt kind, and
turn index remains compatible for run-bound execution receipts. The rebuilt
table adds nullable `prompt_turn_id` and non-null `prompt_link_state`, closed to
`linked_v2`, `legacy_pre_prompt`, or `legacy_unverified`. `linked_v2` requires a
turn foreign key; either legacy state requires it to be null. Every new runtime
receipt write follows actual provider-attempt output/evidence, is `linked_v2`,
and must match the turn tuple. Configuration failure and pre-acceptance
cancellation use only their dedicated tables and never create an
`AcpRuntimeReceipt`. A runtime receipt is never dispatch authority. An original, repair, or continuation receipt cannot overwrite
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

Northbound compatibility is an explicit total projection rather than raw v2
serialization. `P079LeaseCompatibilityProjectionV1` is the only source for the
existing GraphQL `GqlLeaseState` and MCP/report lease `state`,
`settled_result`, and `reclamation_reason` fields:

| Internal v2 state | Existing v1 `state` | Required companion projection |
|---|---|---|
| `reserved` | `reserved` | existing companions unchanged/null |
| `dispatch_pending` | `reserved` | companions unchanged/null; no claim of sent prompt |
| `prompt_sent` | `prompt_sent` | existing companions unchanged/null |
| `dispatch_unknown` | `settled` | `settled_result = delivery_unknown`; existing sanitized reason |
| `settled` | `settled` | preserve the existing terminal result/reason mapping |
| `legacy_unverified` | `settled` | `settled_result = legacy_unverified`; `reclamation_reason = migration_unverified` |

`reports.get` and MCP never call `lease_state.to_string()` for this field, and
GraphQL has no wildcard-to-settled branch; all three invoke this generated
projection. Existing rows in `reserved|prompt_sent|settled` remain byte-equal.
Fixtures cover every v2 state on Operator and redacted non-Operator responses,
reject an unlisted future state, and byte-compare pre-migration v1 rows.

`output_contract_repair_operations_v1` owns logical operation ID, parent
execution/occurrence, selected repair/fallback kind, one permanently consumed
semantic budget, `max_infrastructure_attempts INTEGER NOT NULL DEFAULT 2`, next
infrastructure-attempt index, and terminal result. Each
lease is one attempt and adds non-null operation ID plus attempt index, unique
together. Creating the operation consumes exactly one selected budget; a repair
sets only `repair_budget_consumed`, a fallback only
`fallback_budget_consumed`, and the opposite flag remains false.

Native-v2 logical admission identity is `(repair_event_id, selected_kind)`, not
the caller-pre-generated operation ID. `p079_repair.admit_or_retry` computes
`admission_sha256` over the complete immutable parent/kind/policy intent and
uses one immediate transaction to insert-or-load the native partial-unique tuple before
changing either compatibility budget flag. A concurrent insert with another
operation ID loses the unique constraint, loads the winner, and returns that
same operation/attempt only when the admission digest matches; another digest
is `Conflict`. Budget consumption, operation insert, attempt-0 slot, and the
compatibility event flag commit once. Direct-SQL and two-writer fixtures prove
one surviving native operation and one budget transition for 100 conflicting
IDs. Migration-095 rows do not use that native identity: each valid source lease
keeps one deterministic operation keyed by its source lease, so multiple legacy
same-kind leases for one repair event are preserved without merge.

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
  admission_sha256 TEXT NOT NULL CHECK (length(admission_sha256) = 64),
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
    CHECK (operation_state IN (
      'active','artifact_settlement_pending','settled','legacy_unverified'
    )),
  terminal_result TEXT CHECK (terminal_result IN (
    'accepted','rejected_invalid','skipped_ineligible','unavailable',
    'failed_transport','deadline_exceeded','cancelled','superseded_ignored',
    'lease_contended','budget_exhausted','policy_denied','delivery_unknown',
    'canonical_activation_conflict','legacy_unverified'
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
         (operation_state = 'artifact_settlement_pending' AND
          terminal_result IS NOT NULL AND terminal_result IN (
            'accepted','rejected_invalid','cancelled','superseded_ignored'
          )) OR
         (operation_state = 'settled' AND terminal_result IS NOT NULL AND
          terminal_result <> 'legacy_unverified') OR
         (operation_state = 'legacy_unverified' AND
          terminal_result = 'legacy_unverified'))
);

CREATE UNIQUE INDEX p079_native_logical_admission_unique
  ON output_contract_repair_operations_v1(repair_event_id, selected_kind)
  WHERE source_schema_version = 'native_v2';

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
  UNIQUE (lease_key, work_item_id, prompt_turn_id),
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

CREATE TABLE p079_validation_evidence_v1 (
  validation_evidence_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'p079_validation_evidence_v1'),
  operation_id TEXT NOT NULL,
  attempt_index INTEGER NOT NULL,
  lease_key TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN (
    'accepted','rejected_invalid','unavailable','failed_transport',
    'cancelled','superseded_ignored'
  )),
  candidate_state TEXT NOT NULL
    CHECK (candidate_state IN ('artifact_members','no_candidate')),
  member_count INTEGER NOT NULL CHECK (member_count >= 0),
  membership_sha256 TEXT NOT NULL CHECK (length(membership_sha256) = 64),
  validator_version_sha256 TEXT NOT NULL
    CHECK (length(validator_version_sha256) = 64),
  result_sha256 TEXT NOT NULL CHECK (length(result_sha256) = 64),
  containment_profile_sha256 TEXT NOT NULL
    CHECK (length(containment_profile_sha256) = 64),
  staging_root_identity_sha256 TEXT NOT NULL
    CHECK (length(staging_root_identity_sha256) = 64),
  created_at TEXT NOT NULL,
  UNIQUE (operation_id, attempt_index, lease_key),
  UNIQUE (validation_evidence_id, operation_id, attempt_index, lease_key),
  UNIQUE (
    validation_evidence_id, operation_id, attempt_index, lease_key,
    outcome, member_count, membership_sha256
  ),
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_leases(
      operation_id, attempt_index, lease_key
    ),
  CHECK ((candidate_state = 'artifact_members' AND member_count > 0) OR
         (candidate_state = 'no_candidate' AND member_count = 0 AND
          membership_sha256 =
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855' AND
          outcome IN (
            'unavailable','failed_transport','cancelled','superseded_ignored'
          )))
);

CREATE TABLE p079_no_candidate_witnesses_v1 (
  absence_witness_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'p079_no_candidate_witness_v1'),
  validation_evidence_id TEXT NOT NULL UNIQUE,
  operation_id TEXT NOT NULL,
  attempt_index INTEGER NOT NULL,
  lease_key TEXT NOT NULL,
  reason TEXT NOT NULL CHECK (reason IN (
    'unavailable_before_candidate','failed_transport_before_candidate',
    'cancelled_before_candidate','superseded_before_candidate'
  )),
  staging_entry_count INTEGER NOT NULL CHECK (staging_entry_count = 0),
  staging_scan_sha256 TEXT NOT NULL CHECK (length(staging_scan_sha256) = 64),
  process_terminal INTEGER NOT NULL CHECK (process_terminal = 1),
  prompt_dispatch_state TEXT NOT NULL
    CHECK (prompt_dispatch_state IN ('not_started','prompt_sent')),
  witness_sha256 TEXT NOT NULL UNIQUE CHECK (length(witness_sha256) = 64),
  witnessed_at TEXT NOT NULL,
  UNIQUE (operation_id, attempt_index, lease_key),
  UNIQUE (absence_witness_id, operation_id, attempt_index, lease_key),
  FOREIGN KEY (
    validation_evidence_id, operation_id, attempt_index, lease_key
  ) REFERENCES p079_validation_evidence_v1(
    validation_evidence_id, operation_id, attempt_index, lease_key
  ),
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_leases(
      operation_id, attempt_index, lease_key
    )
);

CREATE TABLE p079_artifact_settlement_sequence_v1 (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1),
  updated_at TEXT NOT NULL
);

INSERT INTO p079_artifact_settlement_sequence_v1(
  singleton_id, next_sequence, updated_at
) VALUES (1, 1, CURRENT_TIMESTAMP);

CREATE TABLE p079_artifact_settlement_sets_v1 (
  artifact_set_id TEXT PRIMARY KEY,
  settlement_sequence INTEGER NOT NULL UNIQUE CHECK (settlement_sequence >= 1),
  operation_id TEXT NOT NULL UNIQUE
    REFERENCES output_contract_repair_operations_v1(operation_id),
  attempt_index INTEGER NOT NULL,
  lease_key TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN (
    'accepted','rejected_invalid','unavailable','failed_transport',
    'cancelled','superseded_ignored'
  )),
  validation_evidence_id TEXT NOT NULL UNIQUE,
  absence_witness_id TEXT UNIQUE,
  member_count INTEGER NOT NULL CHECK (member_count >= 0),
  membership_sha256 TEXT NOT NULL CHECK (length(membership_sha256) = 64),
  state TEXT NOT NULL CHECK (state IN (
    'prepared','history_committed','destination_committed','completed',
    'conflict_settled'
  )),
  prepared_at TEXT NOT NULL,
  history_committed_at TEXT,
  destination_committed_at TEXT,
  completed_at TEXT,
  UNIQUE (operation_id, attempt_index, lease_key),
  UNIQUE (artifact_set_id, settlement_sequence),
  FOREIGN KEY (operation_id, attempt_index, lease_key)
    REFERENCES output_contract_repair_leases(operation_id, attempt_index, lease_key),
  FOREIGN KEY (
    validation_evidence_id, operation_id, attempt_index, lease_key,
    outcome, member_count, membership_sha256
  ) REFERENCES p079_validation_evidence_v1(
    validation_evidence_id, operation_id, attempt_index, lease_key,
    outcome, member_count, membership_sha256
  ),
  FOREIGN KEY (absence_witness_id, operation_id, attempt_index, lease_key)
    REFERENCES p079_no_candidate_witnesses_v1(
      absence_witness_id, operation_id, attempt_index, lease_key
    ),
  CHECK ((member_count > 0 AND absence_witness_id IS NULL) OR
         (member_count = 0 AND absence_witness_id IS NOT NULL AND
          outcome IN (
            'unavailable','failed_transport','cancelled','superseded_ignored'
          ) AND
          membership_sha256 =
            'e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855')),
  CHECK ((state = 'prepared' AND history_committed_at IS NULL AND
          destination_committed_at IS NULL AND completed_at IS NULL) OR
         (state = 'history_committed' AND history_committed_at IS NOT NULL AND
          destination_committed_at IS NULL AND completed_at IS NULL) OR
         (state = 'destination_committed' AND history_committed_at IS NOT NULL AND
          destination_committed_at IS NOT NULL AND completed_at IS NULL) OR
         (state IN ('completed','conflict_settled') AND
          history_committed_at IS NOT NULL AND
          destination_committed_at IS NOT NULL AND completed_at IS NOT NULL))
);

CREATE TABLE p079_artifact_settlement_members_v1 (
  artifact_set_id TEXT NOT NULL
    REFERENCES p079_artifact_settlement_sets_v1(artifact_set_id)
    DEFERRABLE INITIALLY DEFERRED,
  member_ordinal INTEGER NOT NULL CHECK (member_ordinal >= 0),
  logical_output_name TEXT NOT NULL,
  candidate_sha256 TEXT NOT NULL CHECK (length(candidate_sha256) = 64),
  candidate_byte_count INTEGER NOT NULL CHECK (candidate_byte_count >= 0),
  staging_relative_path TEXT NOT NULL,
  history_relative_path TEXT NOT NULL,
  canonical_relative_path TEXT,
  destination_kind TEXT NOT NULL CHECK (destination_kind IN ('published','quarantine')),
  expected_activation_revision INTEGER,
  state TEXT NOT NULL CHECK (state IN (
    'prepared','history_committed','destination_committed','completed',
    'conflict_settled'
  )),
  prepared_at TEXT NOT NULL,
  history_committed_at TEXT,
  destination_committed_at TEXT,
  completed_at TEXT,
  PRIMARY KEY (artifact_set_id, member_ordinal),
  UNIQUE (artifact_set_id, logical_output_name),
  UNIQUE (artifact_set_id, canonical_relative_path),
  UNIQUE (staging_relative_path),
  UNIQUE (history_relative_path),
  CHECK ((destination_kind = 'published' AND canonical_relative_path IS NOT NULL AND
          expected_activation_revision IS NOT NULL AND expected_activation_revision >= 0) OR
         (destination_kind = 'quarantine' AND canonical_relative_path IS NULL AND
          expected_activation_revision IS NULL)),
  CHECK ((state = 'prepared' AND history_committed_at IS NULL AND
          destination_committed_at IS NULL AND completed_at IS NULL) OR
         (state = 'history_committed' AND history_committed_at IS NOT NULL AND
          destination_committed_at IS NULL AND completed_at IS NULL) OR
         (state = 'destination_committed' AND history_committed_at IS NOT NULL AND
          destination_committed_at IS NOT NULL AND completed_at IS NULL) OR
         (state IN ('completed','conflict_settled') AND
          history_committed_at IS NOT NULL AND
          destination_committed_at IS NOT NULL AND completed_at IS NOT NULL))
);

CREATE TABLE p079_artifact_activation_history_v1 (
  run_id TEXT NOT NULL REFERENCES runs(id),
  canonical_relative_path TEXT NOT NULL,
  activation_revision INTEGER NOT NULL CHECK (activation_revision > 0),
  artifact_set_id TEXT NOT NULL,
  member_ordinal INTEGER NOT NULL,
  candidate_sha256 TEXT NOT NULL CHECK (length(candidate_sha256) = 64),
  candidate_byte_count INTEGER NOT NULL CHECK (candidate_byte_count >= 0),
  history_relative_path TEXT NOT NULL,
  activated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, canonical_relative_path, activation_revision),
  UNIQUE (artifact_set_id, member_ordinal),
  UNIQUE (
    run_id, canonical_relative_path, activation_revision, artifact_set_id,
    member_ordinal, candidate_sha256, candidate_byte_count, history_relative_path
  ),
  FOREIGN KEY (artifact_set_id, member_ordinal)
    REFERENCES p079_artifact_settlement_members_v1(artifact_set_id, member_ordinal)
);

CREATE TABLE p079_artifact_activations_v1 (
  run_id TEXT NOT NULL REFERENCES runs(id),
  canonical_relative_path TEXT NOT NULL,
  activation_revision INTEGER NOT NULL CHECK (activation_revision > 0),
  active_artifact_set_id TEXT NOT NULL,
  active_member_ordinal INTEGER NOT NULL,
  active_candidate_sha256 TEXT NOT NULL CHECK (length(active_candidate_sha256) = 64),
  active_candidate_byte_count INTEGER NOT NULL CHECK (active_candidate_byte_count >= 0),
  active_history_relative_path TEXT NOT NULL,
  activated_at TEXT NOT NULL,
  PRIMARY KEY (run_id, canonical_relative_path),
  FOREIGN KEY (
    run_id, canonical_relative_path, activation_revision,
    active_artifact_set_id, active_member_ordinal, active_candidate_sha256,
    active_candidate_byte_count, active_history_relative_path
  ) REFERENCES p079_artifact_activation_history_v1(
    run_id, canonical_relative_path, activation_revision, artifact_set_id,
    member_ordinal, candidate_sha256, candidate_byte_count, history_relative_path
  ),
  FOREIGN KEY (active_artifact_set_id, active_member_ordinal)
    REFERENCES p079_artifact_settlement_members_v1(artifact_set_id, member_ordinal)
);

CREATE TABLE p079_canonical_activation_conflicts_v1 (
  conflict_id TEXT PRIMARY KEY,
  schema_version TEXT NOT NULL
    CHECK (schema_version = 'p079_canonical_activation_conflict_v1'),
  operation_id TEXT NOT NULL UNIQUE
    REFERENCES output_contract_repair_operations_v1(operation_id),
  artifact_set_id TEXT NOT NULL,
  conflicting_member_ordinal INTEGER NOT NULL,
  expected_activation_revision INTEGER NOT NULL CHECK (expected_activation_revision >= 0),
  winning_activation_revision INTEGER NOT NULL CHECK (winning_activation_revision > 0),
  winning_artifact_set_id TEXT NOT NULL,
  winning_member_ordinal INTEGER NOT NULL,
  winning_candidate_sha256 TEXT NOT NULL CHECK (length(winning_candidate_sha256) = 64),
  winning_history_relative_path TEXT NOT NULL,
  canonical_restore_sha256 TEXT NOT NULL CHECK (length(canonical_restore_sha256) = 64),
  conflict_evidence_sha256 TEXT NOT NULL UNIQUE CHECK (length(conflict_evidence_sha256) = 64),
  settled_at TEXT NOT NULL,
  UNIQUE (artifact_set_id, conflicting_member_ordinal),
  FOREIGN KEY (artifact_set_id, conflicting_member_ordinal)
    REFERENCES p079_artifact_settlement_members_v1(artifact_set_id, member_ordinal),
  FOREIGN KEY (
    winning_artifact_set_id, winning_member_ordinal
  ) REFERENCES p079_artifact_settlement_members_v1(artifact_set_id, member_ordinal)
);

CREATE TABLE p079_artifact_reconciliation_checkpoint_v1 (
  singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
  last_completed_sequence INTEGER NOT NULL DEFAULT 0
    CHECK (last_completed_sequence >= 0),
  current_artifact_set_id TEXT,
  current_settlement_sequence INTEGER,
  current_member_ordinal INTEGER,
  current_phase TEXT CHECK (current_phase IN (
    'history_member','destination_member','complete_set'
  )),
  current_evidence_sha256 TEXT
    CHECK (current_evidence_sha256 IS NULL OR
           length(current_evidence_sha256) = 64),
  updated_at TEXT NOT NULL,
  FOREIGN KEY (current_artifact_set_id, current_settlement_sequence)
    REFERENCES p079_artifact_settlement_sets_v1(
      artifact_set_id, settlement_sequence
    ),
  FOREIGN KEY (current_artifact_set_id, current_member_ordinal)
    REFERENCES p079_artifact_settlement_members_v1(
      artifact_set_id, member_ordinal
    ),
  CHECK ((current_phase IS NULL AND current_artifact_set_id IS NULL AND
          current_settlement_sequence IS NULL AND
          current_member_ordinal IS NULL AND
          current_evidence_sha256 IS NULL) OR
         (current_phase IN ('history_member','destination_member') AND
          current_artifact_set_id IS NOT NULL AND
          current_settlement_sequence > last_completed_sequence AND
          current_member_ordinal IS NOT NULL AND
          current_evidence_sha256 IS NOT NULL) OR
         (current_phase = 'complete_set' AND
          current_artifact_set_id IS NOT NULL AND
          current_settlement_sequence > last_completed_sequence AND
          current_member_ordinal IS NULL AND
          current_evidence_sha256 IS NOT NULL))
);

INSERT INTO p079_artifact_reconciliation_checkpoint_v1(
  singleton_id, last_completed_sequence, updated_at
) VALUES (1, 0, CURRENT_TIMESTAMP);

CREATE TABLE p079_migration_quarantine_v1 (
  upgrade_id TEXT NOT NULL,
  source_table TEXT NOT NULL CHECK (source_table IN (
    'output_contract_repair_leases_v1_source',
    'output_contract_repair_fallback_parent_links_v1_source'
  )),
  source_primary_key TEXT NOT NULL,
  source_row_ordinal INTEGER NOT NULL CHECK (source_row_ordinal >= 0),
  source_row_count INTEGER NOT NULL CHECK (source_row_count >= 1),
  source_envelope_json TEXT NOT NULL,
  source_envelope_sha256 TEXT NOT NULL
    CHECK (length(source_envelope_sha256) = 64),
  reason TEXT NOT NULL CHECK (reason IN (
    'dangling_run','dangling_stage','dangling_parent_execution',
    'dangling_fallback_child','dangling_repair_event','dangling_lease',
    'ambiguous_owner','contradictory_budget'
  )),
  source_activity TEXT NOT NULL CHECK (source_activity IN ('active','terminal')),
  parsed_repair_event_id TEXT,
  parsed_run_id TEXT,
  parsed_stage_execution_id TEXT,
  parsed_parent_agent_execution_id TEXT,
  parsed_fallback_agent_execution_id TEXT,
  parsed_lease_key TEXT,
  quarantined_at TEXT NOT NULL,
  PRIMARY KEY (upgrade_id, source_table, source_primary_key),
  UNIQUE (upgrade_id, source_table, source_row_ordinal),
  UNIQUE (upgrade_id, source_table, source_envelope_sha256)
);

CREATE TABLE p079_migration_active_authority_v1 (
  authority_id TEXT PRIMARY KEY,
  upgrade_id TEXT NOT NULL,
  repair_event_id TEXT NOT NULL
    REFERENCES output_contract_repair_events(repair_attempt_id),
  selected_kind TEXT NOT NULL CHECK (selected_kind IN ('repair','fallback')),
  winner_operation_id TEXT NOT NULL,
  winner_attempt_index INTEGER NOT NULL,
  winner_lease_key TEXT NOT NULL,
  cohort_count INTEGER NOT NULL CHECK (cohort_count >= 1),
  cohort_sha256 TEXT NOT NULL CHECK (length(cohort_sha256) = 64),
  selection_reason TEXT NOT NULL CHECK (selection_reason IN (
    'single_eligible_zero_send','ordered_zero_send_winner'
  )),
  selected_at TEXT NOT NULL,
  UNIQUE (repair_event_id, selected_kind),
  UNIQUE (upgrade_id, cohort_sha256),
  FOREIGN KEY (
    winner_operation_id, winner_attempt_index, winner_lease_key
  ) REFERENCES output_contract_repair_leases(
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

CREATE TRIGGER p079_terminal_lease_is_immutable
BEFORE UPDATE ON output_contract_repair_leases
WHEN OLD.lease_state IN ('dispatch_unknown','settled','legacy_unverified')
BEGIN
  SELECT RAISE(ABORT, 'p079_terminal_lease_is_immutable');
END;

CREATE TRIGGER p079_terminal_operation_is_immutable
BEFORE UPDATE ON output_contract_repair_operations_v1
WHEN OLD.operation_state IN ('settled','legacy_unverified')
BEGIN
  SELECT RAISE(ABORT, 'p079_terminal_operation_is_immutable');
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
  SELECT CASE WHEN NEW.terminal_result IN (
    'accepted','rejected_invalid','cancelled','superseded_ignored'
  ) AND NEW.operation_state <> 'artifact_settlement_pending'
    THEN RAISE(ABORT, 'p079_artifact_outcome_requires_pending_set') END;
  SELECT CASE WHEN NEW.operation_state = 'artifact_settlement_pending'
    AND NOT EXISTS (
      SELECT 1 FROM p079_artifact_settlement_sets_v1 s
       WHERE s.operation_id = NEW.operation_id
         AND s.outcome = NEW.terminal_result
         AND s.state = 'prepared'
    ) THEN RAISE(ABORT, 'p079_artifact_set_missing') END;
END;

CREATE TRIGGER p079_artifact_pending_finalize_guard
BEFORE UPDATE OF operation_state ON output_contract_repair_operations_v1
WHEN OLD.operation_state = 'artifact_settlement_pending'
BEGIN
  SELECT CASE WHEN NEW.operation_state <> 'settled'
    OR NEW.terminal_result <> OLD.terminal_result
    OR NOT EXISTS (
      SELECT 1 FROM p079_artifact_settlement_sets_v1 s
       WHERE s.operation_id = NEW.operation_id
         AND s.outcome = OLD.terminal_result
         AND s.state = 'destination_committed'
    ) THEN RAISE(ABORT, 'p079_artifact_settlement_not_committed') END;
END;
```

The full migration additionally installs generated set/member guards. The set
insert verifies contiguous member ordinals `0..<member_count` and the common-
codec SHA-256 of ordered `(ordinal, logical_output_name, candidate_sha256,
candidate_byte_count, destination_kind, history_relative_path,
canonical_relative_path, expected_activation_revision)` tuples. Set and member
state move only `prepared -> history_committed -> destination_committed ->
completed`; each pre-completion set transition requires every member at the
corresponding state. The final set transition additionally requires the
operation already `settled` with the same terminal result and every member
already `completed`. All identity, path, digest, size, destination, and expected-revision
columns remain unchanged. No member or set state may skip or regress.
The sole exception is a zero-member
`unavailable|failed_transport|cancelled|superseded_ignored` set whose
empty membership digest and linked `p079_no_candidate_witness_v1` satisfy the
shown row constraints: only the active Class A
`p079_repair.complete_no_candidate` journal owner may move that set directly
to `completed`, with all three no-filesystem phase timestamps equal to the
single settlement timestamp, in the same transaction that creates the
validation/witness/set and settles operation, lease, item, repair-event
projection, and unchanged parent hold. The generated operation-state trigger
has exactly this journal-owner/zero-member exception and no generic direct
terminal path. It creates no member, history, activation-history, or current-
activation row.
The other explicit terminal branch is `conflict_settled`. Only the active
`p079_repair.settle_activation_conflict` journal owner may move a
history-committed set and all its members to that state after the conflict row
and restored winning canonical digest exist. It simultaneously settles the
operation/event as `canonical_activation_conflict`, preserves the parent hold,
and cannot release a transition. No normal completion transition accepts that
state.

Activation history is append-only and rejects update/delete. The current
activation table rejects direct insert, update, and delete; it is changed only
by the generated activation-history insert trigger while the registered
`p079_repair.commit_destination_member` operation is the active matching
Class A journal owner. That trigger checks the member is `history_committed`,
the run/canonical path/digest/history tuple is exact, and
`activation_revision = expected_activation_revision + 1`; it then performs the
single compare-and-swap from the expected current revision (or inserts revision
1 when expected is zero). A lost CAS is reduced inside the registered operation
to the closed `activation_conflict_observed` variant of
`P079DestinationMemberCommitResultV1`, naming the expected and winning current
activation; it appends no activation history and leaves the member history-
committed. Thus many historical members may name the same canonical path, but
exactly one activation row is current.

Durable P079 evidence is never removable by delete order. Generated triggers
reject every `DELETE` from operation, lease, attempt-slot, attempt-link,
fallback-parent-link, validation-evidence, no-candidate-witness, artifact-set,
artifact-member, activation-history, activation-conflict,
migration-quarantine, and migration-active-authority tables. Attempt
slots/links, validation evidence, no-candidate
witnesses, quarantine rows, and active-authority rows reject every update.
Operations and leases reject
all updates after terminal state; fallback-parent identity is immutable and its
result moves only null-to-one-terminal-value; artifact set/member updates obey
only the transitions above, activation current rows move only through the
history-triggered CAS, and completed rows reject every update. Direct-SQL
fixtures delete children before parents and in reverse order, and mutate every
terminal/result/identity column; every statement is rejected.

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
checked-in migration so the normative table declarations above cannot drift
from the final trigger body.

Migration 100 additionally ships
`docs/reference/schemas/p079-migration-100-sqlite-master-v1.json` as
`P079SchemaManifestV1`. The generated manifest contains the normalized
`sqlite_master` type/name/table-name/SQL tuple and SHA-256 for every P079 table,
index, and trigger, including operations, slots, leases, attempt links,
fallback-parent links, validation evidence, no-candidate witnesses, the
settlement allocator, sets, members, activation history/current activation,
activation conflicts, the reconciliation checkpoint, migration quarantine, and migration active
authority. The tracked SQLx migration and this proposal's generated DDL fixture
must produce that byte-equal manifest from an empty database and from the
migration-095 fixture; an unlisted or missing relation is a schema mismatch.
Startup verifies the installed normalized manifest before any P079 selector or
reconciler can run. Direct mutation fixtures drop/add/rewrite each relation in
turn and prove failed-serve `provider_truth_schema_mismatch`, while a clean
second restart proves no schema rewrite or duplicate seed row.

The operation update guard rejects changes to identity, admission digest,
selected kind, either budget flag, provenance, source schema/key, or attempt
limit. It allows exactly four update shapes: active-to-terminal/non-artifact
settlement; active-to-`artifact_settlement_pending` with a matching prepared
set and complete ordered members; pending-to-settled with matching
destination-committed set/members and unchanged
result; pending-to-settled conflict settlement with a complete conflict row,
all members/set `conflict_settled`, terminal repair event, and unchanged parent;
or same-state active `next_attempt_index = old + 1` with all other semantic fields
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
turn, `linked_v2` attempt-link, contained attached-generation intent,
spawn-pending process intent, and admitted owner binding. It invokes the sealed
`GenerationReservationWriterV1` inside `p079_repair.admit_or_retry`; the repair
never executes in the parent physical provider process. Fallback uses registered Class A
`p079_fallback.admit_or_retry`: it pre-generates child execution, turn,
generation, process-intent, and owner-binding IDs, then in one transaction
validates the parent, inserts operation, attempt-0 slot,
and lease; creates/starts the child AgentExecution and fallback InvokeAgent item
with the validated `production.p079_fallback` envelope; inserts its `original/0`
turn owned by `p079_fallback_child` with owner ID equal to the lease key; then
inserts the `linked_v2` attempt-link and fallback-parent row; and invokes the
sealed `GenerationReservationWriterV1` fragment to insert the initial
contained fresh generation, spawn-pending process intent, and admitted
generation-owner binding.
Its result returns every ID and allocator index. Failure at any
insert or deferred commit check rolls back all rows. No committed lease can
lack its slot/link, and no link can reference a missing work item or turn.
The child has its one initially authorized provider generation when admission
commits; no later claim path may create it. Same-key replay returns the complete
tuple, and concurrent/different-key losers perform zero broker/spawn/provider
I/O. Once that generation is interrupted or
invalidated, the typed P079 owner routes through operation settlement; it never
falls through ordinary InvokeAgent fresh-session or replay policy.

`ProviderAttachProtocolV1` is the shared closed provider-specific attach union
with one currently supported member:
`claude_session_new_resume_session_id_v1` sends one ACP `session/new` whose
`params.resumeSessionId` is that private source ID and requires
`result.sessionId` to prove the resumed identity. This preserves the
already-supported Claude resurrection contract. Codex repair remains zero-send
unsupported until the pinned conformance manifest described above authorizes a
future protocol-version member; an advertised initialize capability alone is
insufficient. No generic attach shape, `session/resume`, `session/load`, alias
fallback, or provider-family inference is legal. P079
repair requires one supported attach branch; P079 fallback uses the ordinary
fresh-session shape inside containment. A provider/adapter without a frozen
supported branch is zero-send `repair_containment_unsupported`.

`P079EnforcedContainmentV1` is mandatory for both repair and fallback prompts.
The only admitted implementation is `macos_seatbelt_staging_v1`; an ACP
permission callback, prompt instruction, provider-reported mode, or
`danger-full-access`/`bypassPermissions` process is advisory and is never a
permit. The daemon creates a mode-0700 operation staging root whose parent and
inode identities it owns plus a separate mode-0700 ephemeral provider-private
state root capped at 4,096 files and 128 MiB, prepares the verified provider launch image above,
and starts that image through the verified `/usr/bin/sandbox-exec` launch
closure. The generated profile is inherited by every descendant and denies all
filesystem writes, renames, links, metadata mutation, and executable creation by
default. It allows writes only beneath the exact daemon-owned staging root and
provider-private state root; the latter may contain adapter/session/cache state
but is never an output candidate, input root, or executable search root and is
purged after bounded evidence extraction. It allows read-only access only to
descriptor-bound workspace/input roots, the verified launch closure, required
system runtime files, and immutable provider configuration. Canonical/history paths and the ordinary workspace are
always write-denied even when they previously were legal output or source roots.
Source-edit-required recovery therefore routes back to implementation; it is
never attempted as P079 repair/fallback.

The contained P079 provider receives an empty MCP server inventory. The adapter
removes every HTTP, SSE, Streamable HTTP, websocket, Xcode/IDE broker, external
socket, and inherited ambient MCP declaration before launch; a profile that
requires one is zero-send `repair_external_tool_authority_unsupported`. No MCP
credentials are resolved. This is stricter than relying on descendant Seatbelt:
an out-of-process service could otherwise mutate the workspace on the
provider's behalf. Provider-created helper processes remain inside the inherited
Seatbelt and receive no daemon/tool socket. Fixtures inject malicious HTTP MCP,
Xcode broker, inherited environment MCP, local socket, and provider-spawned
stdio-child attempts; all external declarations are rejected before resolution,
and every descendant write outside staging/private state is denied by the OS.

Before broker or provider I/O, a runtime probe in that exact child sandbox must
create/fsync/remove test files in staging and private state and must fail attempts to write, rename,
symlink, hard-link, or `openat`-escape into canonical, history, workspace, and a
same-prefix sibling. A child-helper attempt must fail identically, proving
inheritance. Probe evidence stores the sandbox profile digest, staging/private-
state/root inode tuples, launch-image digest, and closed result; it is part of the prompt
permit. Any unavailable `sandbox-exec`, profile parse failure, advisory-only
provider, probe anomaly, path replacement, or denied staging write settles
zero-send. Tests execute direct writes that emit no ACP permission request and
prove the OS still denies them. Thus provider cooperation is not a containment
assumption.

Permit moves the attempt lease/turn to
`dispatch_pending` and sets only `dispatch_started_at`; successful flush plus
final CAS moves both to `prompt_sent` and sets `prompt_sent_at`; ambiguous
delivery moves both to `dispatch_unknown` and immediately runs the typed unknown
settlement; no active item survives that terminal lease.

Before that permit, the admitted operation creates one
`P079RepairOutputBindingV1` from the frozen required-output contract. It names a
runtime-owned operation/attempt staging directory and one deterministic staging
member path per required logical output, while retaining the canonical path only
as non-writable destination metadata. The repair/fallback prompt receives those
staging paths as its resolved output paths. The adapter grants write access only
to that staging directory and explicitly denies every canonical/history root;
the current `p079_repair_canonical_paths` grant and any direct canonical-output
instruction are removed. Provider startup is denied unless the staging dirfd,
ordered member map, permission grant, and operation/attempt digest all match.
Ordinary non-P079 invocations retain their existing output-path behavior.

Provider output itself is never allowed to close only the lease. After bounded
artifact validation, runtime enumerates the contract's complete ordered required
output set and opens only the pre-bound staging members by dirfd-relative no-
symlink traversal. It enforces per-member and aggregate byte caps, computes each
digest while reading, requires a regular file with one link, fsyncs each
candidate and finally fsyncs the staging directory. No database row yet claims
publication. A provider write outside staging, a missing/duplicate/unexpected
member, path collision, symlink, extra link, or changed operation binding fails
validation before set preparation.

The registered Class A `p079_repair.settle_validation` operation is the sole
validation prepare reducer. Its request carries operation/attempt/lease/item/
turn IDs, closed validation outcome, ordered member descriptors, aggregate
member count/digest, validator-version digest, and canonical result digest. For
an artifact-bearing outcome, one transaction verifies the `prompt_sent` turn
and every staged descriptor, writes immutable validation evidence, terminalizes
the item and lease, retains the parent/transition hold, inserts all prepared
artifact members followed by their verified `prepared` set, and moves the
operation to `artifact_settlement_pending` with its terminal result. It does not
claim any member published/quarantined and does not release the parent. It is
never invoked for a candidate-free outcome.

For `unavailable|failed_transport|cancelled|superseded_ignored` before any
candidate exists, the registered Class A
`p079_repair.complete_no_candidate` is the sole reducer. Its closed reason is
respectively `unavailable_before_candidate`,
`failed_transport_before_candidate`, `cancelled_before_candidate`, or
`superseded_before_candidate`. The request binds the operation/attempt/lease/
item/turn/process/parent/event tuple, terminal identity-matched process evidence,
a no-follow scan of the bound staging root proving zero entries, the exact empty
membership digest, and the reason-specific transport/cancellation evidence. One
transaction creates immutable validation evidence, absence witness, and the
already-completed constrained zero-member set; terminalizes item, lease,
operation, and repair-event projection; preserves the original parent result
and transition hold; and records `P079NoCandidateCompletionResultV1`. It performs
no filesystem, history, activation, parent-status, or transition mutation.
Same-key replay returns the complete tuple; a different scan, process identity,
dispatch state, reason, parent/event projection, or evidence digest is
`Conflict`. Crash-before-commit leaves all rows absent, and commit-before-ack is
recovered exclusively from the Class A result. There is no ordinary direct DB
terminal settlement for these outcomes.

A single daemon-owned `P079ArtifactSettlementReconciler` owns prepared sets and
submits only the registered per-member operations in ordinal order.
`p079_repair.commit_history_member` carries the set/member/staging/history/
digest/size tuple. Its filesystem phase opens the already-validated staging and
history roots by directory FD, rejects symlinks/path traversal, rechecks bounded
size/SHA-256, atomically renames to the globally unique versioned history path,
and fsyncs file plus directory. Its DbWriter transaction then records the Class
A result and moves only that member to `history_committed`; the last member also
moves the set to `history_committed`. If staging is absent but history has the
exact digest/size, reconciliation treats the filesystem phase as complete;
different bytes are fatal evidence corruption. Commit-before-ack, file-before-
journal crash, and same-key replay all return
`P079HistoryMemberCommitResultV1` without another rename.

`p079_repair.commit_destination_member` requires the history operation's exact
result. For quarantine, history is the final destination and its DbWriter
transaction advances that member to `destination_committed`; the last member
also advances the set. For published members its filesystem phase creates a
same-directory temporary canonical candidate from the immutable history file
using an inode-independent APFS clone or bounded copy; hard links are forbidden.
It verifies distinct inode identity plus digest/size, fsyncs it, atomically
renames it over the canonical path, and fsyncs
the canonical directory. Its own Class A transaction then appends the
next activation-history row and advances the current activation pointer by the
member's expected-revision CAS before marking that member
`destination_committed`. If the pointer CAS loses, history remains intact, the
operation returns its journaled `activation_conflict_observed` result, and no
transition is released. The reconciler keeps `P079ArtifactActivationGuard`
closed, opens the winning history member by no-follow dirfd traversal, restores
its exact bytes to the canonical path, and submits registered Class A
`p079_repair.settle_activation_conflict`. That transaction verifies the winning
current activation and restored digest, inserts one immutable conflict row,
moves every member and the set to `conflict_settled`, settles operation and
repair-event projection as `canonical_activation_conflict`, and preserves the
parent/transition hold. Persistence failure leaves the guard closed and startup
retries the identical key; it can never remain an untyped active operation.
Same-key replay observing its own activation treats the filesystem step as
complete. `P079ArtifactActivationGuard` excludes canonical-path readers
from the first replacement rename through activation commit or rollback; all
proposal-owned artifact reads open through the activation resolver and verify
the pointed digest after open. A canonical file is authoritative only when its
bytes and the current activation row agree; an unpointed temporary or canonical
replacement is repaired from the pointed history version before the guard
releases or consumers start. File-before-journal, pointer-before-ack, and restart
replay return `P079DestinationMemberCommitResultV1` from exact file/activation/
member truth and never invoke final parent completion.

After every member is destination-committed, the set reaches
`destination_committed`. Registered Class A
`p079_repair.complete_artifact_settlement` performs no filesystem or activation
mutation. It verifies every destination-member result, then in one transaction
updates the operation from `artifact_settlement_pending` to `settled` while the
set is still `destination_committed` (the shown guard's legal precondition),
moves every member to `completed`, moves the set to `completed` under its inverse
guard requiring that settled operation, and only then finalizes repair event/
parent result and releases the matching transition for `accepted`. No other
connection can observe the transaction's temporary operation/set skew. The completed
set, ordered immutable members/history, and current activation pointer are the
durable completion marker. Direct-SQL fixtures prove that exact order succeeds,
while set-first, member-first without settled operation, partial membership,
and restart between any prior committed operation all reject or replay without
an impossible trigger cycle.

Crash fixtures stop before/after the losing CAS observation, winning-history
open, canonical restore, directory fsync, conflict-row/terminal commit, and
acknowledgement. Every restart either completes the original activation or
converges to one typed conflict terminal state with canonical bytes matching the
winning activation, one conflict row/result, no active lease/item/operation,
and no transition release.

The exact reduction is:

| Validation outcome | Prepare transaction | File/reconciliation + completion |
|---|---|---|
| `accepted` | item `completed`; lease `settled(accepted)`; operation `artifact_settlement_pending(accepted)`; parent remains held; complete publish set prepared | commit every candidate to unique history, atomically install canonical bytes, CAS each activation pointer, complete set/operation/event, mark parent repaired, release only matching transition |
| `rejected_invalid` | item `failed`; lease `settled(rejected_invalid)`; operation pending; parent failure/hold retained; complete quarantine set prepared | commit every member to unique quarantine history/destination, complete set/operation/event, keep stage/run blocked |
| `cancelled` or `superseded_ignored` with candidates | item cancelled; matching terminal lease; operation pending; parent terminal truth retained; complete quarantine set prepared | commit every member to unique quarantine history/destination, complete set/operation/event, never release transition |
| `unavailable`, `failed_transport`, `cancelled`, or `superseded_ignored` before a candidate exists | registered `complete_no_candidate` atomically writes reason-specific validation/absence evidence, one completed zero-member set, terminal item/lease/operation/repair event, and unchanged parent hold | no filesystem or activation action; replay is the exact Class A result and no transition is released |
| accepted candidate loses canonical activation CAS | destination operation journals expected/winning activation without changing current truth | restore canonical bytes from winning history; registered `settle_activation_conflict` terminalizes set/members/operation/event as `canonical_activation_conflict`, retains parent hold, and never releases transition |

Fallback-child terminal output uses the same operation-level terminality and
artifact-set protocol through its existing P079 fallback result reducer; it
must close the child InvokeAgent item and lease before pending settlement.
Idempotent replay with the same validation digest returns the complete stored
set/member prepare/completion state; another digest is `Conflict`. Set prepare
allocates monotonic `settlement_sequence` from the shown singleton allocator in
the same transaction using guarded
`UPDATE ... SET next_sequence = next_sequence + 1 ... RETURNING
next_sequence - 1`; exactly one returned row is required and allocator rollback
is coupled to set rollback. Durable
`p079_artifact_reconciliation_checkpoint_v1` stores the
last completed sequence, current set/member/phase cursor, and verified evidence
digest. Before consumers open, each startup attempt runs for at most 10 seconds
of the current boot's continuous clock and processes at most 16 sets, 256
members, and an 8-MiB soft byte budget, ordered strictly by
`(settlement_sequence, member_ordinal, phase)`. A member phase is the indivisible
recovery unit: when the next validated member is larger than the remaining soft
budget, one such member may run alone up to the frozen 10-MiB per-member output
limit plus 2 MiB of bounded authority/copy overhead. The hard per-attempt byte
cap is therefore 12 MiB, and no second member starts after the soft budget is
crossed. A member above the frozen 10-MiB contract limit is terminal invalid
evidence, not a repeatedly retried unit. It checkpoints after each durable
member phase and advances `last_completed_sequence` only after exact final
completion. Reaching any cap keeps failed-serve and the next restart resumes at
the stored cursor; a new set always has a greater sequence and cannot fall below
the checkpoint. The reducer resumes only the exact missing history-member,
destination-member, or final completion operation by its stored replay key and
never rescans a completed sequence. Kill injection covers 0/1/16/17 sets,
0/1/256/257 members, 8-MiB soft and 12-MiB hard byte/time caps, exact 10-MiB
single-member restart convergence, an over-limit member, lower lexical IDs with higher sequences,
and every member candidate
write, file fsync, staging-dir fsync, prepare DB commit/ack, pre/post history
rename, history-dir fsync, canonical temp/clone-or-copy/rename, activation-history
append/current-pointer CAS, canonical-dir fsync, per-member/set state CAS,
completion DB commit/ack, and restart. Every case converges to exactly the
ordered accepted or quarantined member set, one completed set marker, one
current activation per canonical path, no active item/lease/operation, and no
premature parent release. Orphan staged files with no set are never published
and are removed only by bounded age/digest cleanup. Database terminal guards
reject every partial direct-SQL ordering.

Budget consumption is never refunded. A TTL-expired `reserved` attempt with
turn still `not_started` terminalizes its linked item and settles that attempt
`deadline_exceeded`; the
explicit two-attempt infrastructure allowance may atomically allocate attempt
`n` only by inserting the guarded slot at the operation's current
`next_attempt_index`; its trigger advances the allocator exactly once without
consuming another logical budget.
For repair this creates a fresh repair item/turn; for fallback it uses the same
ordered atomic admission routine to create the lease, fresh child
AgentExecution, InvokeAgent item, original turn, attempt-link, parent link,
initial generation, process intent, and generation-owner binding.
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
`dangling_run`, `dangling_stage`, `dangling_parent_execution`,
`dangling_fallback_child`, `dangling_repair_event`, `dangling_lease`,
`ambiguous_owner`, or `contradictory_budget`,
nullable parsed correlation IDs, active/terminal source classification, and
timestamp. `dangling_fallback_child` is used when a migration-095 fallback row's
child execution is absent or does not match its lease/parent tuple; it is never
collapsed into parent ambiguity. The typed envelope records every source column in declaration order as
`{name, sqlite_type, value}`; text bytes are UTF-8, integers use canonical
decimal, null is tagged, and blobs are base64. The quarantine table has no FK to
any potentially dangling source identity. It is writeable only by
`ProviderTruthUpgradeCoordinator` while holding `PreflightLockGuard`.

Only rows whose mandatory run, stage, event, parent execution, lease, and
fallback-child relationships all validate may create canonical operation,
slot, lease, link, or fallback-parent rows. Every other source row is copied to
quarantine and digest-verified before the source table can be dropped. Final
accounting requires, independently for the lease source and fallback-parent
source tables,
`source_count = canonical_source_count + quarantine_count`, disjoint source
keys, and byte-equal source-envelope digests. An active quarantined row keeps
bootstrap failed with its sanitized reason; a terminal quarantined row is
retained for diagnostics but is never exposed to dispatch/replay selectors.
`foreign_key_check` must be empty because quarantine intentionally carries no
domain FKs. Crash/restart fixtures stop before and after each quarantine write,
canonical write, count/digest checkpoint, source swap, and final FK check.

Two or more migration-095 leases with the same repair event and selected kind
remain distinct historical `p079_migration_095` operations whose
`source_lease_key` values are unique; no source row is merged or discarded.
Migration 100 additionally creates append-only
`p079_migration_active_authority_v1` with unique
`(repair_event_id, selected_kind)`, winner operation/lease IDs, the complete
cohort digest/count, selection reason, and selected time. Update/delete are
rejected. Active selectors require this exact authority row and may return only
its winner.

`P079MigrationActiveAuthorityReducerV1` classifies the full same-event/same-kind
cohort before any member can remain `reserved`. If any member has positive or
ambiguous provider-I/O/delivery evidence, the reducer creates no authority row:
each row reaches its classifier-prescribed `dispatch_unknown`, terminal, or
quarantine state, and the parent stays blocked. If every cohort member is
provably zero-send, it filters through the complete eligibility reducer and
chooses at most one winner by the immutable total order
`(lease_acquired_at as canonical timestamp, lease_key UTF-8 bytes)`. The winner
alone may preserve/become `reserved`; every other valid zero-send member settles
`superseded_ignored` with its item/turn terminalized before authority is
published. If no member is eligible, no authority is written. A row with a
dangling mandatory identity still goes only to typed quarantine and participates
in cohort accounting/digest but can never win.

The reducer and authority insertion run in the guarded migration transaction
after all source rows are staged but before tracked-schema swap. Restart
recomputes the same bounded cohort digest and total order; a differing winner,
missing loser settlement, or changed cohort is corruption. Fixtures shuffle
input order and cover one/two/many same-kind rows, timestamp ties, one dangling
child, all eligible, all ineligible, positive I/O, ambiguous I/O, and restart at
every winner/loser/authority checkpoint. They prove preserved per-source
history/accounting, at most one dispatch-capable operation, and no budget rewrite.

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
because they share a parent, but only the authority-selected operation may be
dispatch-capable. The upgrader stages rows by
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
merely because they share a parent execution. Duplicate active operations pass
through `P079MigrationActiveAuthorityReducerV1`; a reserved selector without the
matching immutable authority row is rejected. Invalid mandatory identity is
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

Migration 100 rebuilds `provider_sessions` around an opaque immutable
`provider_session_ref_id` primary key and moves the provider's raw wire session
ID into private one-to-one `provider_session_private_ids`. Fresh refs are random
UUIDv4 values with `psref_` prefix; legacy migration checkpoints one random ref
per old raw-PK row before rebuilding every FK, so restart reuses the stored ref
instead of deriving it from secret bytes. The private table stores the raw wire
ID only for ACP resume, is reachable solely through a crate-private
`ProviderSessionSecretResolver` capability, and is excluded from generic repos,
debug derives, tracing, artifacts, new authority JSON codecs, and all new
northbound schemas. The capability has exactly two call purposes:
`provider_wire`, which returns a zeroizing buffer scoped to one
`session/resume` serialization, and `legacy_projection`, which may populate only
an already-existing authorized raw-ID compatibility field. New private
acceptance/receipt authority JSON uses `provider_session_ref_id`.

Frozen `AcpRuntimeReceipt` v1 and every existing authorized GraphQL/MCP/report
`provider_session_id` field keep their historical nullable raw-provider-ID
meaning byte-for-byte. Migration never rewrites historical bytes. New P086
runtime receipts follow that same raw-or-null contract through the explicit
`legacy_projection` capability; they never place a `psref_` value in a raw-ID
field. The opaque ref is stored only in private relational authority columns and
is absent from GraphQL, MCP, report, Swift, and artifact schemas. This proposal
adds no public `providerSessionRefId` field.

The already-public P046 GraphQL field `SessionGeneration.providerSessionRef`
is a separate compatibility projection and is not the durable
`provider_session_ref_id`. Its wire contract remains exactly 32 lowercase hex
characters: the first 16 bytes of
`SHA256("p046_v1|psr|" || run_id || "|" || process_instance_salt || "|" ||
raw_provider_session_id)`. The salt is the existing random 16-byte
process-instance value, so the projection is stable only within one daemon
process and deliberately changes across restarts. After migration the resolver
may obtain the raw value only through `ProviderSessionSecretResolver` purpose
`legacy_projection`, immediately derive the P046 value, and discard the
zeroizing raw buffer. It may never return, hash, or reinterpret the relational
`psref_*` value. Authorization and null/redaction behavior remain byte-equal to
P046. The fixed vector run
`00000000-0000-0000-0000-000000000600`, raw value
`provider-session-secret-abc123`, and sixteen zero salt bytes yields
`14e115dcea7fb3290e60358e693c7f68`; a second fixed salt yields a different
value, while pre/post-migration resolution with the same fixed salt is
byte-identical. Source inventory permits this field only in the generated P046
compatibility manifest and fails on any new `derive_scoped_ref(..., "psr")`
caller or any northbound `psref_` value.

Migration is driven by a generated `ProviderSessionCorrelationManifestV1`, not
only declared foreign keys. It inventories every schema column and Rust/Swift
call site that stores, compares, hashes, logs, serializes, or accepts
`provider_session_id`. The mandatory schema rows are `provider_sessions`,
`session_generations`, `agent_work_continuations`,
`provider_cancellation_intents`, `shutdown_interrupted_receipts`,
`shutdown_signal_side_effects`, and cancellation late-output rows including
their normalized generated/index columns. Each durable correlation is rebuilt
to `provider_session_ref_id` and copy-verified through the private mapping;
provider wire requests resolve the raw secret only at the ACP boundary. Legacy
command inputs that still carry a raw ID pass through a private compatibility
resolver and are normalized to the stored ref before authority comparison.
Unclassified source/schema hits fail the gate and preflight. Fixtures cover
every manifest row plus old/new command, shutdown, cancellation, continuation,
generation, and late-output paths.

Migration 100 creates append-only `provider_session_resume_contexts` with
primary key `context_id`, literal schema version
`provider_session_resume_context_v1`, unique source-generation FK, internal
opaque provider-session-ref FK, provider, adapter-contract version, target-binding
fingerprint, exact attach-protocol tag, duplicate-free ordered
`ProviderRootAuthorityV1` IDs plus their common-codec digest and display-path
projection, immutable MCP descriptor-set reference/digest, canonical context
JSON/digest, and creation time. Context JSON contains the provider-session row reference but
not the provider-session secret or expanded broker secrets. Insert triggers
recompute the canonical digest and verify every referenced row; update/delete
are rejected. `agent_work_continuations` adds nullable
`provider_session_resume_context_id` plus digest. Resurrection admission requires
both non-null and matching; live-handle mode requires both null; output-only
requires null until its one-way resurrection conversion atomically installs the
pair together with its first resurrection-window FK. Cross-generation,
cross-provider, changed-root, changed-MCP, and
current-workspace recomputation attempts fail before launch. Sentinel fixtures
place a distinctive raw session ID in the private map and prove it is absent
from new authority JSON/digests, context, logs, errors, artifacts, snapshots,
generated test diagnostics, and every non-compatibility projection. Separate
compatibility fixtures prove the existing authorized raw-ID fields still expose
exactly raw-or-null according to their prior caller policy, never `psref_*`, and
that unauthorized callers receive their existing redacted/null form.

Migration 100 creates append-only `p086_resurrection_windows_v1`. Each row has
`resurrection_window_id` as its primary key, unique non-null `continuation_id`,
non-null unique `reconciliation_sequence` allocated from a database singleton in
the admission/conversion transaction,
closed `source = initial_admission | output_only_conversion`, the source Class A
operation/journal identity, exact resume-context ID/digest, and a
`DurableResurrectionWindowClockV1`. That clock stores the immutable macOS
`boot_session_id` read from `kern.bootsessionuuid`,
`opened_continuous_ns` from `mach_continuous_time` converted with the cached
`mach_timebase_info`, literal `setup_duration_ns = 30000000000`, literal
`setup_cleanup_duration_ns = 10000000000`, and checked derived
`setup_deadline_continuous_ns` and `setup_cleanup_deadline_continuous_ns`.
Immutable
`opened_at`, `setup_deadline_at`, and `setup_cleanup_deadline_at` RFC 3339 fields are
display/audit estimates only and are never used for admission or expiry.
Insert triggers verify the duration equations, checked integer arithmetic,
source/operation agreement, and continuation/context tuple; update and delete
are rejected. On the same boot, every comparison uses a fresh
`mach_continuous_time` sample, so sleep counts and wall-clock jumps are
irrelevant. A different boot-session ID makes an unprompted setup window expired
unconditionally: startup may perform identity-safe cleanup and zero-prompt
settlement only, never broker acquisition, spawn, resume, configure, or prompt.
A continuation whose canonical turn is already `prompt_sent` has left setup and
is governed by its frozen ordinary execution watchdog; reboot does not rewrite
sent truth or convert it to zero-send setup cleanup.

A continuation stores nullable `resurrection_window_id`. Initial resurrection
admission inserts and binds exactly one window. Live-handle and initial
output-only rows require it null. Output-only conversion may change that FK
only once from null to the newly inserted conversion window; neither the window
nor any clock field is rewritten afterward. Migration classifies every active
legacy `provider_session_resurrection` row without a complete V1 monotonic
clock as `legacy_resurrection_window_unverifiable`; after identity-safe cleanup
it settles failed-closed with zero provider work. It does not infer a deadline
from wall time, create a replacement window, or convert the row to live-handle
or output-only mode. Terminal legacy rows remain read-only. Same-boot boundary,
sleep, forward/backward wall jump, reboot, and incomplete-legacy fixtures freeze
the reducer result and prove no provider call on reboot/legacy cleanup.

P086 admission pre-generates the continuation, ProcessContinuation work-item,
prompt-turn, attached-generation, process-intent, owner-binding, and resurrection-
window IDs. The setup deadline bounds only broker acquisition, launch-gate
release, initialize, attach, configuration reverification/persistence, prompt
permit, transport write/flush, and the final `prompt_sent` CAS. It never bounds
model execution or terminal response after that CAS. The setup-cleanup deadline
bounds identity-safe reap and durable zero-send settlement only when setup has
not reached `prompt_sent`. One
registered Class A `p086_continuation.admit` DbWriter operation performs command
idempotency and policy checks and, for resurrection, atomically inserts the
command-journal row, continuation with the context/window tuple, immutable window, Pending work
item, allocated `not_started` turn, `provider_send` side-effect row in
`reserved`, active configuration attempt `0`, pre-session generation, process
binding in `spawn_pending` with launch nonce, window boot-session ID, and
current parent daemon PID but no child PID, and generation-owner
binding in `admitted`. The owner next-attempt allocator advances to `1`; current
receipt remains null and evidence is `pending`. The work item stores non-null
run/stage owner fields. A transaction-body error rolls back the entire tuple;
an acknowledged accepted response therefore always names one claimable item,
turn, generation, process intent, and both setup deadlines. A timeout after
transaction start returns `Unknown` and reconciles by command-journal ID before
any response, claim, broker call, or spawn. Identical replay returns every
stored ID plus the same immutable window/monotonic clock tuple and cannot enqueue, allocate,
or extend either clock again.

Broker/toolchain acquisition, process spawn/barrier, `initialize`, the exact
provider-specific `ProviderAttachProtocolV1` operation, configuration
reverification, receipt persistence, and
the final prompt-write CAS consume only the remaining setup duration on the
window's boot; no phase resets either clock. At setup-deadline expiry before
`prompt_sent`, no further broker/provider/prompt I/O is allowed. Cleanup first reconciles the exact
configuration-settlement Class A operation/key stored on this owner binding,
before any process signal, reap, absence claim, or configuration terminal
projection. A committed failure is replayed. A committed receipt/readiness is
preserved. Only after the journal result is known, or rollback/no-start is
proved, may cleanup settle a never-launched intent or perform the parent-aware,
identity-safe process action above. While the prompt turn is still
`not_started`, cleanup then appends exactly one
`provider_post_configuration_outcomes_v1` row with the class-appropriate
`configured_deadline_before_prompt` or
`provider_ready_deadline_before_prompt`; it never creates a conflicting failure or
cancellation ref. A proved no-start configuration attempt may settle the typed
`configuration_deadline_elapsed` failure. Journal absence after an
uncertain-after-start observation, a receipt/failure conflict, or an
unreconciled write closes first fatal and remains failed-serve. Cleanup-deadline
expiry with unresolved process/authority likewise closes first fatal and leaves
the tuple for startup reconciliation; it never detaches a cleanup task or grants
more time.
Startup claims each nonterminal tuple by persisted phase and its window. A turn
before `prompt_sent` may continue setup only before the setup deadline,
otherwise it performs only the remaining setup cleanup. A `prompt_sent` turn
skips the setup-expiry reducer and resumes the ordinary sent-turn recovery and
frozen execution watchdog. After setup-cleanup expiry for an unprompted turn,
`P086ExpiredWindowReconcilerV1` runs
inside a five-second `mach_continuous_time` sub-budget of the existing startup
safety window. It processes at most 32 continuations and 1 MiB of bounded
authority/evidence bytes per transaction in ascending
`reconciliation_sequence` order. Durable
`p086_expired_window_reconciliation_checkpoint_v1` stores the last fully
settled sequence and evidence digest; a budget/cap stop keeps failed-serve and
the next restart resumes strictly after that sequence. Boot-session UUID and
continuous time remain evidence fields, never ordering keys, so a later boot ID
that sorts lexically below an earlier one still receives a greater database
sequence. It first reconciles the exact
configuration journal, then may inspect the recorded launch identity, perform
the parent-aware identity-safe process action, and settle the existing window,
generation, and continuation rows. It may not acquire a broker, spawn,
initialize, resume, attach, configure, reserve another turn or generation, or
send a prompt. Proved absence or a proved reap terminalizes the continuation
`failed_closed(expired_resurrection_window)`; ambiguous identity or an
uncommitted terminal settlement keeps failed-serve. Repeated restart retries
only this same cursor-addressed cleanup reducer and therefore converges without
provider work, rescanning completed keys, or a replacement deadline. Fixtures
cover 0/1/32/33 rows, byte-cap truncation, budget expiry, restart continuation,
PID reuse, parent death, a different boot session, and a lower-sorting next boot
UUID with a higher reconciliation sequence.

Live-handle continuation uses the same admission operation but binds its
validated existing generation and derives a continuation-owned attempt/receipt
without allocating a process intent. Output-only begins in that branch. Its
one-way conversion to resurrection is a separate Class A CAS and never rebinds
the already-bound output-only turn. Before commit, the source turn must still
be `not_started`, its source `provider_send` side effect must still be
`reserved`, the source generation-owner binding must have no possible-delivery
evidence, and cancellation/supersession must be absent. The transaction appends
one immutable `p086_output_only_conversion_causes_v1` row with closed cause
`live_handle_lost | operator_recovery`, source operation/turn/side-effect/
generation/binding IDs, context digest, and observed zero-send evidence. It
leaves the source turn immutable in `not_started`, appends the class-appropriate
typed post-readiness outcome
`configured_superseded_for_resurrection` or
`provider_ready_superseded_for_resurrection`, terminalizes the source side
effect and owner binding by reference to that immutable outcome, and allocates the next
turn index as a replacement `work_continuation_output_only` turn, still under the
frozen output-only contract but attached through resurrection, with a new
reserved provider-send side effect at the next checked continuation-local
side-effect sequence number.

Inside that same registered
`p086_continuation.convert_output_only_to_resurrection` transaction, the private
`GenerationReservationWriterV1` inserts the continuation's first and only
resurrection window from the conversion operation's monotonic admission sample,
binds its context/window tuple, and creates the new turn's active attempt,
pre-session generation, spawn-pending process intent, and admitted binding. The
`P086ResurrectionConversionResultV1` contains every source and replacement ID,
the cause-row ID, and the exact clock/window tuple. Cancellation before commit
rolls back the replacement tuple and settles only the source continuation under
normal cancellation policy; cancellation after commit targets the replacement
turn/generation and cannot reopen the source. Same-key replay returns the same
old/new tuple; another source operation, context, cause, source state, or clock
is `Conflict`. Neither path points at the target execution's receipt, invokes
the ordinary reservation operation, allocates after provider launch, or mutates
the old prompt-turn binding in place.

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
`ResurrectionPhaseV1` is frozen to the ten migration-081 values `admitted`,
`launching`, `launched`, `attaching`, `attached_unprompted`, `prompting`,
`settling`, `cancelling`, `completed`, and `failed_closed`.
`ResurrectionPhaseV2` is the internal closed nullable enum containing those ten
plus `configuration_reverified`, `prompt_dispatch_pending`, and `prompt_sent`.
Migration accepts only a V1 token from the old column and maps each shared token
byte-identically; the three new values are native-v2 outputs and are never
legacy inputs.
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
command/idempotency evidence, terminal runtime-receipt evidence, and whether a
complete V1 monotonic resurrection window exists on the current boot. Reduction
precedence is closed: positive correlated post-I/O evidence, possible delivery,
terminal no-replay, unverifiable/expired resurrection cleanup, provable
non-resurrection zero-send, then invalid/contradictory quarantine.

| Historical evidence | v2 result |
|---|---|
| Live-handle or output-only mode; no send row; continuation accepted/queued; valid bound work item; no positive I/O evidence | `reserved` with a newly allocated `not_started` turn |
| Live-handle or output-only mode; old `planned`; otherwise same unique owner tuple | `reserved`; old row alone is zero-send evidence |
| Active legacy provider-session-resurrection without a complete V1 current-boot monotonic window | identity-safe cleanup then `failed_closed(legacy_resurrection_window_unverifiable)`; no turn replay and no provider work |
| Old `started` or `committed`, continuation `prompt_sent`, or any contradictory active combination without positive post-I/O evidence | `dispatch_unknown`, owner quarantine, no replay |
| Old `released`, release durably precedes any I/O boundary, phase is no later than `attached_unprompted` or `configuration_reverified`, and identity-matched process absence is proved | `failed` with `not_started` turn; zero-send retry follows only the continuation policy |
| Old `released` with missing ordering, phase `prompting` or later, live/ambiguous process identity, or any positive I/O evidence | `dispatch_unknown`, owner quarantine, no replay |
| Unique owner plus runtime receipt with positive post-I/O prompt timestamp and matching provider session/request fingerprint | `prompt_sent`, linked turn, no replay |
| Terminal continuation with no active replay path | Preserve terminal status; map ledger only for readback, never enqueue |
| Missing/duplicate owner, mismatched attachment, failed send row, or contradictory terminal evidence | `failed` or `dispatch_unknown` according to possible bytes, quarantine, no replay |

An active migrated live-handle/output-only row may remain zero-send `reserved`
only during same-process online upgrade when its ProcessContinuation item is
uniquely bound, worker lease and heartbeat are current, the supervised-handle
registry contains the non-cloneable live handle owned by the current daemon
process boot, process start identity matches the attach receipt, release is
absent, phase is `attached_unprompted` or `configuration_reverified`, terminal
idempotency evidence is absent, and no old value implies possible I/O. Startup
after process death can never satisfy the handle predicate: it performs the
parent-aware identity action and settles proved zero-send as failed, or fails
closed on ambiguous/possible delivery; it never leaves the row active or
reconstructs a handle from PID/session metadata. A stale/missing worker or
released lease with an identity-matched process is terminated/observed absent
then failed; ambiguous process identity quarantines without signalling. Any terminal command-journal
or continuation result dominates queue status and prevents resurrection. A
terminal idempotency row with non-terminal queue/ledger state converges to that
terminal result and never re-enqueues. No migrated active
`provider_session_resurrection` row is eligible for that branch because legacy
storage has no V1 monotonic authority; only a native v2 admission or explicit
output-only conversion can create a resurrection window.

The old engine writes `committed` and continuation `prompt_sent` before ACP I/O;
those values are therefore never positive evidence. Only the correlated
post-I/O receipt row above can produce migrated `prompt_sent`. A table-driven
generator enumerates the full finite cartesian product of every classifier axis
above for all three modes; invalid state-dependent tuples must reduce to typed
invalid/quarantine rather than being omitted. It asserts one classification,
turn/link nullability, process action, owner settlement, restart idempotency,
and replay exclusion for every generated row.

The legacy phase reducer is executable and exhaustive over exactly the ten V1
tokens; each maps to its byte-identical shared V2 phase. It rejects any old-row
token outside V1. Dispatch truth is then reduced independently from the
ledger, receipt, process, release, and terminal axes above; phase alone never
proves send or zero-send. Checked-in goldens are produced independently by
`scripts/reference/p086_provider_truth_upgrade.py`, a stdlib-only implementation
that imports neither Rust code nor generated migration output. Mutation cases
remove each of the ten old phases and `released` state in turn and must fail
coverage.

Existing MCP and GraphQL resurrection-phase fields remain V1. The sole
`P086ResurrectionPhaseCompatibilityProjectionV1` maps every internal V2 value:
the ten shared values are identity mappings; `configuration_reverified` maps to
`attached_unprompted`, `prompt_dispatch_pending` maps to `prompting`, and
`prompt_sent` maps to `settling`. The detailed authoritative prompt turn remains
the source for sent truth, so this projection never upgrades delivery. MCP and
GraphQL invoke that total generated projection and reject an unlisted V2 value;
they never serialize the internal string directly. Fixtures byte-compare all V1
rows and cover all three native-only values.

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
not a post-serve task. Production daemon startup first calls exactly
`db::bootstrap::acquire_runtime_database_lock(database_url)`, whose closed outer
result is:

| Lock outcome | Meaning / action |
|---|---|
| `acquired(PreflightLockGuard)` | This process owns bootstrap and may bind its listener in `starting` mode |
| `duplicate_healthy(ExistingOwnerEvidenceV1)` | PID/start identity and authenticated health identify the already-serving owner; this process never binds or opens DB and exits with typed duplicate-owner status |
| `anomalous_holder(ExistingOwnerEvidenceV1)` | Lock holder exists but identity/health is stale, mismatched, or unverifiable; no signal, takeover, listener, or DB open occurs; operator recovery is required |
| `lock_failure(SanitizedLockFailureV1)` | No ownership proof; process exits without serving or mutating |

Only `acquired` enters
`db::bootstrap::open_runtime_database_with_guard(database_url, guard)`. That
inner API returns the closed `RuntimeDatabaseBootstrapOutcomeV1` union:

| Outcome | Owned value | Serve transition |
|---|---|---|
| `ready` | `RuntimeDatabase { pool, preflight_lock_guard }` | `starting -> normal` only after every preflight proof succeeds |
| `preflight_failed` | `FailedBootstrapOwner { preflight_lock_guard, sanitized_failure, failure_code }` | `starting -> failed`; no writable/readable runtime pool exists |

Both inner owner values are non-`Clone` and non-serializable. In particular,
`FailedBootstrapOwner` retains the live `PreflightLockGuard` until process exit,
so a failed serve process cannot accidentally release singleton ownership and
race another local opener. No outer duplicate/anomalous/failure result pretends
to own a guard. Daemon `supervisor` never acquires a second database lock and
never retries bootstrap in-process. Recovery requires a clean process restart.

The singleton lock uses a stable inode. Setup creates the mode-0600 lock file if
absent, but normal `PreflightLockGuard::drop` only unlocks/closes its file
descriptor and never unlinks, renames, truncates through another descriptor, or
replaces the path. While holding exclusive `flock`, acquisition verifies that
path and descriptor still name the same device/inode, rewrites bounded owner
evidence through that descriptor, and fsyncs it. A path/inode mismatch is
`anomalous_holder`, not takeover. Process death releases the kernel lock on the
same persistent inode. A retained three-process race pauses owner A during drop
while B and C contend; exactly one successor acquires the same inode, the other
sees its healthy owner, and no two guards or writable pools coexist. A source
scan rejects `remove_file`/unlink of the production lock path.

The lock, principal table, daemon launch images, and private runtime homes live
under one no-symlink mode-0700 control root in
`~/Library/Application Support/Chainworks Forge/control-plane-private`; the
baseline Seatbelt profile installed for every provider and descendant denies
read, write, rename, unlink, link, metadata, and directory enumeration against
that root. Provider credentials are delivered through the one-shot resolver
above, never by granting the child access to the principal/auth path. Startup
verifies owner UID, every parent/leaf mode, link count one, device/inode, and no
ACL granting another identity. A permissive, replaced, linked, or provider-
reachable control root is `lock_failure`, not a repair or takeover. Direct and
descendant provider fixtures attempt pathname replacement and lock-record
mutation under full adapter permissions and are denied by the OS.

`ExistingOwnerEvidenceV1` is produced only by a complete authenticated
challenge, never by PID liveness or unauthenticated `/health`. While the guard
is held, the owner writes a bounded `runtime_lock_owner_v1` record through the
locked descriptor containing lock device/inode, boot-session ID, PID/start
identity, daemon generation, exact loopback endpoint, random owner nonce, an
ephemeral Ed25519 public key, and a certificate over that key and tuple made by
an installation key in the macOS Keychain. The installation-key access control
requires the signed Chainworks daemon designated requirement; its private key
is neither in the file nor available to provider processes. The health handler
answers `ownerChallengeV1` only while its live non-cloneable guard still owns
that exact flock. A contender supplies a fresh 256-bit nonce and the observed
lock tuple; the signed response covers schema, request nonce, owner nonce,
lock tuple, boot session, PID/start, daemon generation, endpoint, and a fresh
`mach_continuous_time` sample.

The contender accepts `duplicate_healthy` only when the certificate chain,
response signature, exact request nonce, endpoint peer, lock descriptor/path
identity, process executable signature, PID/start/boot tuple, and a maximum
two-second challenge round trip all match. A prior response is never reusable;
an invalid, missing, delayed, replayed, path-replaced, unrelated-live-PID, or
correctly signed but wrong-lock response is `anomalous_holder`. Fixtures freeze
known-answer signatures and cover lock-path replacement behind a held flock,
owner-record replay, endpoint redirection, PID reuse, process restart, boot
change, stale nonce, wrong install key, and an unrelated healthy daemon. None
may bind/open the DB or signal a process.

Inside that API, `run_preflight_with_guard(&mut PreflightLockGuard)` returns a
private `PreflightCompleteToken` only after migration, Rust finalization, and
reconciliation succeed. `create_pool_after_preflight(database_url, token)`
consumes that token and opens the runtime pool without calling preflight or
reacquiring the lock. The ordinary `create_pool` remains only for in-memory
tests and explicitly feature-gated maintenance binaries; a retained production
call-site scan rejects it in daemon startup. One-shot admin commands use the
same outer acquisition and inner bootstrap path and keep the returned ready or
preflight-failed owner until exit.

The registered SQLx migration 100 is deliberately a staging migration: it
creates `provider_truth_upgrade_state`, shadow/final-target tables with nullable
backfill columns, and compatibility read views, but installs no constraint that
requires Rust-derived data. `ProviderTruthPhasedMigratorV1` runs under the guard
with an explicit branch:

1. A custom ledger preflight compares every applied version/checksum against the
   full embedded source, rejects an applied version above the binary maximum,
   and reads provider-truth phase without invoking SQLx's missing-version check.
2. If phase is already `complete`, it skips the filtered migrator entirely and
   runs only the ordinary full embedded `Migrator`; this is the clean-restart
   path after 101+ has been applied.
3. Otherwise, preflight requires no applied version above 100, constructs a
   filtered SQLx `MigrationSource` containing exactly versions `<= 100`, and
   runs it. Migration 100 becomes ledger-visible but no 101+ SQL executes.
4. The Rust coordinator resumes or starts migration-100 finalization, verifies
   the final schema, and durably marks phase `complete` while holding the guard.
5. Only after a fresh complete read does the full embedded `Migrator` run;
   applied `<= 100` entries are checksum-verified/skipped and pending `> 100`
   migrations execute in order.

The filtered source is constructed from the same embedded migration bytes and
checksums as the full source, not copied SQL. A tracked-equal DB with migration
100 applied but incomplete phase cannot enter the complete-phase branch or
bypass Rust finalization. A synthetic migration
101 fixture records that its SQL side effect and `_sqlx_migrations` row are both
absent at every migration-100 crash point, then appear exactly once after final
phase completion. A second clean restart after 101 proves the complete-phase
branch skips the filtered source and accepts the full ledger unchanged.
An applied version above 100 while phase 100 is still incomplete is not repaired
or adopted: it is typed operator-only corruption
`provider_truth_future_migration_applied_before_phase_complete`, retains the
failed bootstrap owner, and performs zero finalizer, rollback, migration, or
consumer work. Recovery requires an operator-provided database restore or a
later explicitly versioned adoption migration; this proposal defines neither an
in-place rollback nor history rewrite. A two-restart negative fixture preserves
the same ledger/side effects and refusal on both attempts.

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

A real three-process fixture opens the same file-backed database. Process A
acquires the lower-layer guard, completes tracked-equal and subset finalization,
opens its pool through `create_pool_after_preflight`, and keeps serving without
self-reacquiring the flock. Process B cannot enter migration or open a writable
pool and receives the existing duplicate/anomalous-holder outcome. Process C
races B while A drops or is killed. At each preflight checkpoint the kernel
releases the same persistent-inode lock; exactly one of B/C resumes the durable
marker and the other observes that owner. Trace assertions show one lock acquisition
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
  "provider_session_ref_id": "psref_...",
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
`mediation_record_id`, nullable `p079_operation_id`, nullable
`p079_attempt_index`, nullable `p079_lease_key`, nullable `continuation_id`,
`work_item_id`, non-null `session_generation_id`, non-null
`provider_session_ref_id`, `provider`, non-null `binding_fingerprint_sha256`,
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

Receipt owner kind is closed to `agent_execution`,
`p017_mediation_execution`, `p079_repair_attempt`, `p086_continuation`, or
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
matches the owning execution row. For `p017_mediation_execution`, execution and
mediation record are non-null while occurrence/stage/continuation are null. For
`p079_repair_attempt`, parent execution/occurrence and operation/attempt/lease
are non-null, owner ID is the lease key, and the parent execution's own current
configuration pointer is unchanged. For `p086_continuation`, continuation,
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
| `invoke_agent`, `p058_escalation` | `agent_execution` | The prompt's exact execution/occurrence; P058 special authority also matches |
| `p017_mediation` | `p017_mediation_execution` | Exact stage-less mediation execution/record/work item/run epoch; occurrence and stage remain null |
| `p079_repair` | `p079_repair_attempt` | Operation, attempt, lease, repair work item, turn, generation, and immutable parent execution/occurrence all match; cancellation cannot mutate the parent |
| `p079_fallback_child` | `agent_execution` | The lease-bound child execution/occurrence and typed fallback provenance all match |
| `p086_continuation` | `p086_continuation` | Continuation, target execution/occurrence, attach receipt, work item, and generation all match; target receipt pointer is unchanged |
| `steward_agent_lane` | `steward_agent_lane` | Lane, analysis, agent, work item, lineage, and generation all match with null run/execution fields |

This owner map is crossed with the generated provider-contract matrix:

| Provider contract class | Pre-prompt success authority | Failure/cancel authority | Accepted model/effort |
|---|---|---|---|
| `codex_exact_pair` | owner-scoped configuration receipt settled by `provider_configuration.settle_success` | typed configuration failure/cancellation before acceptance; typed receipt-qualified post-readiness outcome after acceptance | non-null exact verified pair |
| `provider_neutral` | unique generation readiness plus owner/attempt/turn binding settled by `provider_configuration.settle_readiness` | typed provider-start failure/cancellation before readiness; typed readiness-qualified post-readiness outcome afterward | always null; requested pair remains planned only |
| `legacy_best_effort` | unique legacy generation readiness plus owner/attempt/turn binding settled by `provider_configuration.settle_readiness` | typed legacy-start failure/cancellation before readiness; typed readiness-qualified post-readiness outcome afterward | always null/unverified |

Every legal provider/capability/prompt-owner cell is generated from these two
tables into SQL checks, the class-tagged permit union, both Class A success
settlement codecs, reservation/dispatch/cancellation reducers, GraphQL/
Swift readback fixtures, and startup selectors. P079 repair and P086 may reserve
only an existing/attached generation and fail closed on mismatch; they never
convert a missing class-appropriate authority into an ordinary fresh session.
Adding a provider class or prompt owner without a complete cell is a compile/
gate failure. Restart and direct-SQL negatives cross every cell, especially
non-Codex readiness, stage-less P017, and P079 cancellation.

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
Only private relational authority may differ; all public projections remain
byte-compatible and no opaque ref is added northbound.

### Configuration and prompt-turn dispatch lifecycle

The engine inserts a fresh exact Codex execution with requested fields and
`provider_configuration_state = configuring` before ACP startup. Claim/start
atomically creates the execution and its `original/0` turn in
`not_started`; non-Codex and legacy executions receive the same original row
with non-applicable/unverified configuration truth. For Steward,
`run_steward_analysis_with_executor` loads the pre-inserted analysis and two
lane rows, then threads the lane ID, claimed StewardAnalysis work-item ID, and
already committed prompt-turn ID into each `StewardAgentInvocation`; the
executor validates and loads that exact turn before calling ACP and has no
turn-allocation permit. It does not manufacture
a RunId, StageExecution, or AgentExecution as authority. A strict owner-aware
provider-configuration sink on
`AcpRuntimeManager`:

- after both option responses are verified, atomically writes generation
  acceptance and the owner receipt, then projects it and marks configuration
  `configured` when the owner is an AgentExecution;
- on negotiation failure, writes `failed_before_prompt` with null accepted
  fields plus one `provider_configuration_failures` row and no receipt;
- on cancellation that wins before configuration completes, writes
  `cancelled_before_configuration` plus one
  `provider_configuration_cancellations` row,
  creates no receipt, keeps the original turn `not_started`, and marks the
  execution/work item cancelled in the same settlement transaction;
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

Before the outer database lock, daemon may only call
`PrincipalTable::read_existing_no_mutation`. That function opens an already-
present absolute principals file by no-follow dirfd traversal and returns its
inode/mode/digest snapshot, or the typed value `absent`; it never creates a
directory/file, chmods, rotates credentials, repairs bytes, or starts a watcher.
Daemon then obtains the outer database-lock outcome above.
`duplicate_healthy`, `anomalous_holder`, and `lock_failure` destroy the read-only
snapshot and exit without changing auth paths, DB bytes, listeners, or runtime
homes. Only `acquired(PreflightLockGuard)` can derive the non-cloneable
`PrincipalBootstrapOwnerPermitV1`, bound to lock device/inode, boot session,
daemon PID/start identity, and acquisition nonce. Under that permit,
`load_or_bootstrap_owned` either reopens and byte-compares the existing snapshot
or atomically creates the first mode-0600 principal file and parent directory;
only then does it install the live reloadable table/watcher.

After owned principal initialization succeeds, daemon constructs one Axum
router, binds one listener, and starts it with
`RuntimeServeLifecycleV1 = starting` before running inner preflight. An auth
bootstrap failure exits before bind and cannot publish an unauthenticated
starting router. Three-process first-boot fixtures start with no auth directory
and prove exactly the acquired owner creates one credential while every
non-acquired process leaves auth/DB/listener state byte-identical. The lifecycle
is the closed one-way state
machine `starting -> normal | failed` and `normal -> failed`; `failed` is
terminal for the process. In `starting`, the outer middleware bypasses all
normal resolvers and permits only unauthenticated `/health` and `/ready`
returning 503, the exact authenticated GraphQL `daemonStatus` operation, typed
MCP refusal, and sanitized 503 for every other route. On
`RuntimeDatabaseBootstrapOutcomeV1::ready`, daemon installs the runtime owner
and atomically publishes `normal`. On `preflight_failed`, it stores the entire
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
must contain exactly `query`, `operationName`, and `variables`, with
`operationName = "P031DaemonStatus"` and `variables = {}`. These are the exact
bytes structurally emitted by production `P031URLSessionGraphQLReadTransport`.
The document must contain exactly one named query operation
`P031DaemonStatus`, no GraphQL variable definitions, extensions, fragments, aliases,
arguments, directives, inline fragments, mutation, or subscription, and this
exact root selection:

```graphql
query P031DaemonStatus {
  daemonStatus {
    json
  }
}
```

The implementation must migrate the shipped `DaemonLifecycleClient`, not only a
test helper. It deletes the client's handwritten/unnamed `daemonStatus` request
and injects `P031URLSessionGraphQLReadTransport`, invoking exactly
`P031GraphQLDocumentSet.daemonStatus`. The resulting production HTTP body has
the named document above, `operationName = P031DaemonStatus`, and an empty
variables object. A source gate rejects any other production `daemonStatus`
document, raw URLSession body, unnamed operation, or alternate polling path.
Hosted transport tests drive the real lifecycle client against both `starting`
and `failed` minimal routers, assert successful repeated polling for a live
Operator principal, and assert the standard refusal after live-table revoke,
disable, or scope removal.

The minimal handler parses JSON and GraphQL with the same parser/version as the
normal server, canonicalizes only insignificant whitespace/comma placement,
and compares operation kind, name, field names, and tree shape to this AST.
Malformed bodies, non-empty/non-object/missing `variables`, another operation
name, missing/extra/duplicate field, mixed
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
| `original` | P017 mediation execution attempt ID | Stage-less mediation-owned execution, mediation record, conflict, InvokeAgent item, and captured run epoch all match; stage/occurrence are null | Fail attempt/item; move mediation to `terminal_unverifiable(prompt_delivery_unknown)`; retain blocked conflict for operator settlement; never mutate a stage owner |
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
never an owner token. It also owns one monotonic `ConfigurationDeadline` per
configuration attempt from allocation onward. The setup deadline is 30 seconds
from allocation and is never reset. Spawn/barrier
release, Xcode broker/toolchain acquisition, `initialize`, `session/new`, every
configuration write/readback, authority persistence, and outcome handoff each
run under `tokio::select!` against that deadline, the generation token, and the
current owner token. Timeout/cancellation records the exact last phase; the
manager performs identity-safe cleanup and invokes authority zero-send
settlement before returning, under one distinct monotonic 10-second setup-cleanup
deadline established together with the setup deadline. Ordinary attempts create
both deadlines at allocation; P086 loads the exact bound boot-session ID plus
`setup_deadline_continuous_ns` and `setup_cleanup_deadline_continuous_ns`, and
rejects unprompted setup work when the current boot ID differs. Its RFC 3339 deadline fields are
display-only. Cleanup is
not an extension of setup work: after the setup deadline only reap and
zero-send settlement are legal. An identity-ambiguous child is
quarantined rather than signalled. No broker request, authority call, settlement
await, transport task, or cleanup task may detach or outlive its owning
deadline. For P086 resurrection, the pair is reconstructed from its immutable
window row; launch/attach/restart never creates a replacement window. Configuration
settlement uses a CAS over captured owner truth. Prompt dispatch then holds the
gate from permit through its transport write/flush deadline and final CAS.
Ordinary owners use a fixed 10-second deadline. P086 computes one immutable
dispatch deadline as
`min(setup_deadline_continuous_ns, write_start_continuous_ns + 10000000000)`;
permit validation, transport write, flush, and the final CAS all receive that
same bound and may not start when it is already expired. At or after the P086
setup deadline, proven no-write settles zero-send and any positive or ambiguous
write/flush observation settles `dispatch_unknown`; cleanup may not grant a
fresh write interval. A committed `prompt_sent` turn
does not release the gate: the generation-owner binding moves to
`awaiting_terminal`, and the same non-cloneable guard remains held through the
provider terminal response, terminal runtime-receipt persistence, and owner
settlement. For P086, successful final CAS irreversibly leaves the resurrection
setup window. The terminal-response wait is bounded only by the existing frozen
execution watchdog (currently the ordinary 300/900-second class selected for
that agent/task). Attach/configuration/prompt-write calls cannot restart, but
provider response reads and the normal cancellation channel may continue under
that watchdog. If the execution watchdog expires after durable `prompt_sent`, the manager preserves sent
truth, identity-safely terminates/observes the generation, and settles the
existing P086 runtime failure oracle as `provider_runtime_timeout_after_prompt`
with no output-only retry. The remaining cleanup interval permits only process
observation/reap and DB settlement under the ordinary terminal-cleanup deadline
created at watchdog/cancellation settlement; it is not the expired setup-cleanup
clock. Any final settlement/cleanup await is bounded and cannot hang daemon shutdown. A
second logical owner may be durably admitted and configured, but it
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
with both applicable tokens and the owner-specific dispatch deadline above and
reports `zero`, `some`, or `unknown` bytes written. The coordinator waits at
most that same deadline for the gate, then asks the
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
| P079 repair | Settle attempt `unavailable`, fail the linked `OutputContractRepair` item, preserve operation budget and parent execution, and block for existing P079 recovery policy | Zero; the admitted contained generation forbids transparent fresh fallback and automatic replay |
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
| PID/start identity persisted; barrier released; no correlated fresh-session result or provider-specific attach result (`Codex session/resume` / `Claude session/new.resumeSessionId`) | Identity-check and reap child, settle configuration failure | No reuse of old generation |
| Correlated fresh-session or provider-specific attach result exists, or configuration is `configuring`; turn `not_started` | Identity-check and reap, write `failed_before_prompt`; ambiguous identity quarantines owner | No P079/P086 fresh fallback |
| Configured receipt committed; turn `not_started`; daemon lost transport | Reap old generation; an ordinary owner may use only its frozen recovery policy, Steward retains receipt A and terminalizes `configuration_failed(configured_transport_lost_before_dispatch)` without consuming/allocating its zero-send retry, and P079/P086 fail closed | Never reuse old receipt as new-generation truth |
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

P079 repair must use its atomically admitted contained generation and exact
provider-specific attach branch; P086 must use its admitted live or
provider-specifically attached generation. Neither owner kind may fall through
to the manager's generic fresh-session path. Fresh-session creation is available
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
  cancelled_before_configuration configured_terminated_before_prompt
  legacy_unverified
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
  agent_execution p017_mediation_execution p079_repair_attempt
  p086_continuation steward_agent_lane
}
enum ProviderConfigurationFailureCode {
  model_unavailable model_not_accepted effort_unavailable effort_not_accepted
  acceptance_persistence_failed provider_start_failed
  provider_process_identity_unverified configuration_deadline_elapsed
  resume_unsupported resume_configuration_unavailable
  configuration_evidence_invalid
}
enum ProviderConfigurationCancellationCode {
  cancelled_before_configuration
}
enum ProviderPostConfigurationOutcomeCode {
  configured_cancelled_before_prompt configured_deadline_before_prompt
  configured_transport_lost_before_dispatch
  configured_superseded_for_resurrection
  provider_ready_cancelled_before_prompt
  provider_ready_deadline_before_prompt
  provider_ready_transport_lost_before_dispatch
  provider_ready_superseded_for_resurrection
}
enum ProviderConfigurationAcceptanceSource {
  fresh_negotiation reused_session_generation attached_session_reverification
}
enum RuntimeReceiptLinkState {
  linked_v2 legacy_pre_prompt legacy_unverified
}
enum ProviderConfigurationEvidenceState {
  pending receipt_available readiness_available invalidated failure_available
  cancellation_available not_applicable legacy_unverified
}
enum ProviderPromptDeliveryTruth {
  not_started original_pending original_sent repair_pending repair_sent
  continuation_pending continuation_sent steward_pending steward_sent unknown
  legacy_unverified
}
enum ProviderPromptTurnFailureCode {
  configuration_failed prompt_preparation_failed owner_cancelled_before_prompt
  owner_superseded_before_prompt provider_generation_interrupted_before_prompt
  prompt_transport_failed prompt_delivery_unknown
  provider_runtime_failed_after_prompt provider_runtime_timeout_after_prompt
  provider_generation_interrupted_after_prompt legacy_authority_unverifiable
}
enum TimelineLaneKind {
  occurrence run_events
}
enum TimelineIdentityState {
  matched_occurrence_v2 unassociated_run_event
}
enum TimelineHistoryCompleteness {
  complete legacy_gap
}
enum TimelineLegacyGapReason {
  pre_journal_history_unavailable
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
  configurationFailureCode: ProviderConfigurationFailureCode
  configurationCancellationCode: ProviderConfigurationCancellationCode
  postConfigurationOutcomeCode: ProviderPostConfigurationOutcomeCode
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
  failureCode: ProviderPromptTurnFailureCode
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
  configurationFailureCode: ProviderConfigurationFailureCode
  configurationCancellationCode: ProviderConfigurationCancellationCode
  postConfigurationOutcomeCode: ProviderPostConfigurationOutcomeCode
  runtimeReceiptLinkSummary: RuntimeReceiptLinkSummary!
  promptDispatchSummary: ProviderPromptDispatchSummary!
  promptTurns: [ProviderPromptTurn!]!
}
type FrozenPresentationProviderIdentity {
  provider: String!
  model: String
  effort: String
  identityDigest: String!
}
extend type QueryRoot {
  providerExecutionTruthSchemaVersion: Int!
}
extend type GqlAgentExecution {
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  presentationRowId: ID
  presentationProviderIdentity: FrozenPresentationProviderIdentity
  providerExecutionTruth: ProviderExecutionTruth!
}
extend type RunStageTopologyOccurrence {
  presentationRowId: ID!
  compiledTaskId: ID!
  taskOccurrenceId: ID
  occurrenceSequence: Int
  occurrencePosition: TopologyOccurrencePosition!
  presentationProviderIdentity: FrozenPresentationProviderIdentity!
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
  timelineLaneEventOrdinal: Int!
  presentationProviderIdentity: FrozenPresentationProviderIdentity
  durableEventSequence: String!
  canonicalEventDigest: String!
  durableTimelineCursor: String!
  projectionGeneration: Int!
  gapDetected: Boolean!
}
type RuntimeTimelineSnapshot {
  schemaVersion: String!
  snapshotCursor: String!
  upperEventSequence: String!
  snapshotDigest: String!
  handoffCursor: String!
  nextPageCursor: String
  hasMore: Boolean!
  historyCompleteness: TimelineHistoryCompleteness!
  legacyGap: RuntimeTimelineLegacyGap
  events: [GqlRuntimeEvent!]!
}
type RuntimeTimelineLegacyGap {
  schemaVersion: String!
  reason: TimelineLegacyGapReason!
  sourceRowCount: Int!
  sourceDigest: String!
  earliestReliableCursor: String
}
enum CursorPageState { more exhausted }
enum RunStageTopologyEntryKind { occurrence transition }
type RunStageTopologyStageHeader {
  stageId: ID!
  frozenWorkflowOrdinal: Int!
  legacyOrderUnverified: Boolean!
  stageDigest: String!
}
type RunStageTopologyPageEntry {
  entryOrdinal: Int!
  kind: RunStageTopologyEntryKind!
  stageId: ID!
  occurrence: RunStageTopologyOccurrence
  transition: RunStageTopologyTransition
}
type RunStageTopologyPage {
  schemaVersion: String!
  topologySnapshotCursor: String!
  topologySnapshotDigest: String!
  pageState: CursorPageState!
  nextCursor: String
  stageHeaders: [RunStageTopologyStageHeader!]!
  entries: [RunStageTopologyPageEntry!]!
}
type OccurrenceExecutionAttemptPage {
  schemaVersion: String!
  topologySnapshotCursor: String!
  topologySnapshotDigest: String!
  pageState: CursorPageState!
  nextAttemptCursor: String
  attempts: [GqlAgentExecution!]!
}
extend type QueryRoot {
  runtimeTimelineSnapshot(
    runId: ID!, snapshotCursor: String, afterCursor: String, first: Int!
  ): RuntimeTimelineSnapshot!
  runStageTopologyPage(
    runId: ID!, topologySnapshotCursor: String,
    afterCursor: String, first: Int!
  ): RunStageTopologyPage!
  occurrenceExecutionAttemptPage(
    runId: ID!, taskOccurrenceId: ID!, topologySnapshotCursor: String!,
    attemptAfterCursor: String, first: Int!
  ): OccurrenceExecutionAttemptPage!
}
# Final additive argument list on the existing SubscriptionRoot field:
# runtimeStatusChanged(
#   runId: ID, replayCursor: String, durableAfterCursor: String
# ): GqlRuntimeEvent
extend type GqlTimelineRawDetailResult {
  timelineEventId: ID
  agentExecutionId: ID
  taskOccurrenceId: ID
  taskOccurrenceSequence: Int
  presentationRowId: ID
  timelineLaneId: ID
  timelineLaneKind: TimelineLaneKind
  timelineIdentityState: TimelineIdentityState
  timelineLaneEventOrdinal: Int
}
```

The checked-in complete SDL snapshot must contain that final subscription field
signature exactly as
`runtimeStatusChanged(runId: ID, replayCursor: String,
durableAfterCursor: String): GqlRuntimeEvent`; the comment above represents the
replacement signature of the existing field, not a second field or an SDL
extension shortcut.

`RuntimeTimelineSnapshot.schemaVersion` is exactly
`runtime_timeline_snapshot_v1`. Null, another literal, or omission is a schema
error. `snapshotCursor` is only the immutable pagination snapshot identity;
`handoffCursor` is the durable `timeline_cursor_v1` upper-bound cursor consumed
by the subscription. They are not interchangeable. Cursor-mode validation is
exact: `durableAfterCursor` requires non-null
`runId` and null `replayCursor`; legacy `replayCursor` requires null
`durableAfterCursor`; both null preserve the existing live-only subscription
behavior. Both cursors non-null, a malformed/cross-run durable cursor, or an
unauthorized run returns the exact typed GraphQL errors defined below before
registering a receiver. A valid cursor below
retained history emits one existing `requiresFullRefetch = true` control frame
and closes. Legacy `sequenceCursor`, replay-window behavior, and old query bytes
remain unchanged for clients that omit the new argument.

Every Rust enum in this delta declares
`#[graphql(rename_items = "snake_case")]`; its GraphQL literal is exactly the
lowercase snake-case token shown above. The SDL snapshot and resolver fixtures
send every legal lowercase literal and reject uppercase, mixed-case, unknown,
and future values. No default async-graphql enum rename convention may silently
change this wire vocabulary.

Timeline request failures use one domain schema and two exact transport
envelopes. Every GraphQL `errors[]` entry has a bounded human `message` and the
normal `path` when available. HTTP query errors have `extensions` exactly
`{"schemaVersion":"timeline_graphql_error_v1","code":"<CODE>",
"requestId":"<existing P042 request id>"}`; missing/empty `requestId` is a
transport contract failure. GraphQL-over-WebSocket `next.payload.errors[]`
entries have `extensions` exactly
`{"schemaVersion":"timeline_graphql_error_v1","code":"<CODE>"}` and are bound
to the non-empty outer protocol operation `id`; they do not invent an HTTP
request ID. Extra extension keys are rejected in both transports. The closed
domain code set is `TIMELINE_RUN_ID_INVALID`, `TIMELINE_CURSOR_MODE_INVALID`,
`TIMELINE_SNAPSHOT_CURSOR_INVALID`, `TIMELINE_PAGE_CURSOR_INVALID`,
`TIMELINE_DURABLE_CURSOR_INVALID`, `TIMELINE_CURSOR_RUN_MISMATCH`,
`TIMELINE_PAGE_SIZE_INVALID`, `TIMELINE_RUN_UNAUTHORIZED`, and
`TIMELINE_SNAPSHOT_EXPIRED`, and
`TIMELINE_SNAPSHOT_ANCHOR_PERSISTENCE_FAILED`. Snapshot and subscription resolvers parse every
supplied `runId` with the strict canonical `RunId` parser before authorization,
cursor lookup, replay lookup, receiver registration, or any database read. A
malformed supplied ID is always `TIMELINE_RUN_ID_INVALID`; it can never become
the optional no-filter branch. Cursor syntax, mode, run binding, page size,
authorization, and snapshot lifetime are then checked in that order, yielding
exactly one listed code. Query and WebSocket fixtures cover every code and prove
zero receiver/replay registration for every rejection. `RunId` lexical grammar
is exactly a 36-byte lowercase hyphenated UUID v4 in `8-4-4-4-12` layout: the
version nibble is exactly `4`, the RFC 4122 variant high bits are exactly `10`,
and the parsed UUID round-trips byte-for-byte through the canonical lowercase
formatter. Nil, uppercase, compact, braced, `urn:uuid:`, leading/trailing
whitespace, a noncanonical hyphen layout, another version/variant, or malformed
hex is `TIMELINE_RUN_ID_INVALID` before any downstream work.

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
owner's current class-specific receipt/readiness/failure/cancellation/post-
outcome projection; only the receipt-plus-post-outcome row is a legal two-ref
combination. Each turn child instead joins
`provider_generation_owner_bindings.prompt_turn_id` to the exact configuration
owner, attempt, generation, class-specific readiness/receipt/failure/
cancellation/post-outcome, and invalidation evidence for that turn.
It never guesses from the owner's current pointer.

For a new turn, configuration owner kind/ID, attempt index, provider, requested
pair, and evidence state are present. A planned non-Codex shell with no
execution/attempt uses `not_applicable`; an execution-bearing non-Codex turn
uses `pending`, `readiness_available`, `failure_available`, or
`cancellation_available` with null configuration state and accepted fields. A migrated
turn that cannot be linked uses `legacy_unverified`; owner/attempt/generation
and configuration fields may then be null. This table is the exhaustive
readback algebra for execution and turn projections:

| Contract/state | Evidence state | Accepted pair | Failure code | Cancellation code | Post-config outcome | Pre-prompt `AcpRuntimeReceipt` |
|---|---|---|---|---|---|---|
| planned exact-Codex topology shell, no execution | `pending` | null | null | null | null | absent |
| exact `configuring` | `pending` | null | null | null | null | absent |
| exact `configured` | `receipt_available` | complete | null | null | null | absent |
| exact `invalidated_after_acceptance` | `invalidated` | complete historical | null | null | null | absent iff turn not sent |
| exact `failed_before_prompt` | `failure_available` | null | exactly one typed value | null | null | absent |
| exact `cancelled_before_configuration` | `cancellation_available` | null | null | `cancelled_before_configuration` | null | absent |
| exact `configured_terminated_before_prompt` | `receipt_available` | complete | null | null | exactly one typed value | absent |
| planned provider-neutral shell, no execution/attempt | `not_applicable` | null | null | null | null | absent |
| provider-neutral `admitted`, configuration state null | `pending` | null | null | null | null | absent |
| provider-neutral ready, configuration state null | `readiness_available` | null | null | null | null | absent before prompt |
| provider-neutral failed before readiness, configuration state null | `failure_available` | null | exactly one typed value | null | null | absent |
| provider-neutral cancelled before readiness, configuration state null | `cancellation_available` | null | null | `cancelled_before_configuration` | null | absent |
| provider-neutral terminal after readiness, configuration state null | `readiness_available` | null | null | null | exactly one matching `provider_ready_*` value | absent |
| legacy `legacy_unverified` | `legacy_unverified` | null | null | null | null | preserve only historical terminal receipt behavior |

The planned shell requires `agentExecutionId`, owner, attempt, generation,
requested, accepted, and all code fields null; its planned provider/model/effort
come only from the adjacent frozen topology occurrence fields. It has a
non-null `not_started` prompt summary, an empty turn array, and cannot be
constructed for a row that already has an execution. The complete
owner/attempt/generation tuple is required for every execution-bearing
non-legacy exact row. `invalidated` additionally requires invalidation evidence. The three
code columns are GraphQL enums and Swift closed enums, not strings. Any other
combination is `provider_configuration_truth_invalid`; resolvers do not reduce
it. A terminal `AcpRuntimeReceipt` may be inserted only after the authoritative
turn reached `prompt_sent|dispatch_unknown`; configuration failure,
pre-configuration cancellation, and post-configuration zero-prompt outcome
fixtures prove no runtime-receipt row exists.

Consequently a P079 repair shows original turn receipt/attempt A on source
physical generation A and repair turn receipt/attempt B on a separately
contained physical generation B attached to the same logical provider-session
identity. The parent
execution's current receipt pointer remains receipt A for its entire lifetime;
receipt B is reachable only through the exact repair lease/attempt binding and
its repair-turn child. Cancellation, restart, and latest-specialized-turn
reducers may move or terminalize the lease pointer but may never project B onto
the parent. The original turn continues to join receipt A and its terminal
historical snapshot. A P086
target execution instead retains original physical generation A while the
continuation turn exposes continuation-owned attached generation B; admission
never updates the target pointer. Resolver fixtures assert both receipt pairs,
owners, attempts, physical generations, and sources simultaneously. A P079
option invalidation observed during repair invalidates future reuse of source
generation A and contained repair generation B, but never rewrites the already
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
stage, provider, timestamp, or latest-execution lookup.

Every durable event also has non-negative `timeline_lane_event_ordinal`.
Migration 100 creates `timeline_lane_event_allocators_v1(run_id,
timeline_lane_id, next_ordinal)` with primary key `(run_id, timeline_lane_id)`.
New-event persistence CAS-allocates exactly the current ordinal and increments
the counter in the same transaction that inserts the event, before publication.
For each complete-envelope legacy row that is eligible for copying, migration
assigns ordinals independently per lane by the immutable tagged total order:
valid timestamps use `(0, CanonicalUtcTimestampV1 bytes, runtime_event_id UTF-8
bytes)` and unparseable legacy timestamps use `(1, runtime_event_id UTF-8
bytes)`. It never sorts unparseable raw timestamp bytes. Migration checkpoints
the source count/order digest so restart is byte-identical. The
ordinal is presentation/readback identity only and is not added to the frozen
`runtime_event_id` hash. After backfill it seeds every allocator to verified
`max(ordinal) + 1`; a run with only gap evidence seeds no lane allocator.
Duplicate/skipped ordinals or a counter below the copied-row bound fail
preflight.

Migration 100 also creates append-only `runtime_timeline_events_v1`. A single
registered Class A `runtime_timeline.persist_event` DbWriter transaction
allocates global non-negative `event_sequence`, allocates
the lane ordinal above, inserts the complete bounded `GqlRuntimeEvent` source
fields plus run/authorization owner, non-null `canonical_event_sha256`, and a non-null unique
`durable_timeline_cursor`, and only then publishes the event to the subscription
bus. Update/delete are rejected. The durable cursor is
`timeline_cursor_v1:<64 lowercase hex>` over common-codec components
`[run_id, canonical_event_sequence, runtime_event_id]`; it is never a process-
local broadcast counter.

`event_sequence` is a SQLite signed 64-bit integer constrained to
`0...9223372036854775807`; allocation at the upper bound fails closed before an
insert. The wire projects it as canonical unsigned ASCII decimal with no sign
and no leading zero except the single byte `0` as `durableEventSequence`, never
GraphQL `Int`, and projects the stored 64-lowercase-
hex digest as `canonicalEventDigest`. The digest domain is
`chainworks.runtime_timeline_event.v1` over duplicate-key-rejected RFC 8785 JSON
of every transported event field except `projectionGeneration`, `gapDetected`,
`requiresFullRefetch`, and the digest itself; it includes the sequence, durable
cursor, complete identity/lane tuple, authorization owner, timestamp, bounded
detail metadata, and legacy compatibility fields. Rust persistence, GraphQL
query/subscription, and Swift independently recompute the same known-answer
vectors. A row with an unparsable/overflowing sequence, changed field under the
same sequence/event ID, or mismatched digest is corruption/full-refetch, never
deduplicated success. Existing non-null `projectionGeneration` and `gapDetected`
remain selected and decoded on every snapshot/subscription event.

The migration never fabricates a complete event envelope from the existing
`timeline_raw_details` subset or from process-local broadcast history. It copies
only a source row that already contains every authoritative event, identity,
authorization, timestamp, lane, and bounded-detail field and passes the exact
envelope validator. The pre-migration schema has no durable history-completeness
marker, so every run present when migration 100 starts receives a
`pre_journal_runtime` gap row even when its raw-detail row count is zero; copied
complete-envelope rows establish a reliable suffix but cannot erase that gap.
Only a run created after the journal-ready migration fence may begin as
`complete`. For each affected run/source whose pre-journal rows cannot be
reconstructed, migration inserts one immutable
`runtime_timeline_legacy_gaps_v1` row keyed by `(run_id, source_name)` with
literal reason `pre_journal_history_unavailable`, source row count, ordered
source digest, nullable earliest reliable event sequence/cursor, and migration
time. Update/delete are rejected. Migration processes at most 256 source rows,
4 MiB, or 15 seconds per guarded startup pass and checkpoints the last source
key/count/digest; until every copy-or-gap decision is complete and verified,
Timeline consumers remain failed-serve. Raw-detail-only, mixed complete/incomplete,
empty, malformed, crash-per-row, and restart fixtures prove no fabricated event
and byte-identical gap evidence.

Migration 100 also creates
`runtime_timeline_snapshot_leases_v1(snapshot_cursor PRIMARY KEY, run_id,
upper_event_sequence, snapshot_sha256, expires_at, created_at)` and
`runtime_timeline_empty_cursor_anchors_v1(durable_timeline_cursor PRIMARY KEY,
snapshot_cursor UNIQUE REFERENCES runtime_timeline_snapshot_leases_v1,
run_id, upper_event_sequence, snapshot_sha256, expires_at, created_at)` with
unique `(run_id, upper_event_sequence, snapshot_sha256)`. Identity/digest fields
are immutable. At a snapshot bound with no
retained event for the run, `TimelineEmptyAnchorWriterV1` derives the cursor with
the same codec using that run, the frozen upper sequence, and an exact empty
`runtime_event_id`; it uses registered Class A
`runtime_timeline.ensure_empty_anchor` and commits or replays the anchor before the snapshot response
can expose that cursor. No synthetic event row is inserted. Durable cursor
lookup is the exact union of the unique event cursor and unique empty-anchor
cursor; zero or two matches is corruption. A first event committed after the
anchor has a greater global sequence and is replayed exactly once after process
restart. Anchor commit-before-response, event races on either side of the bound,
server crash, retention expiry, and repeated empty snapshots prove no gap or
duplicate; anchor persistence failure returns
`TIMELINE_SNAPSHOT_ANCHOR_PERSISTENCE_FAILED`, never an unusable cursor.

Snapshot/anchor lifetime is bounded. A new lease expires exactly 15 minutes
after its server-issued UTC creation timestamp, that expiration is MAC-bound in
both snapshot and handoff cursors, and an anchor copies the same value. At most
64 unexpired empty anchors per run and 4096 globally may exist; creation first
runs bounded expiry pruning and fails with
`TIMELINE_SNAPSHOT_ANCHOR_PERSISTENCE_FAILED` rather than evicting an unexpired
anchor. Registered Class A `runtime_timeline.prune_empty_anchors` is the sole
delete authority. It deletes at most 64 expired anchor/lease pairs or 1 MiB per
transaction in `(expires_at, durable_timeline_cursor)` order, first inserts one
append-only `runtime_timeline_anchor_prune_tombstones_v1` row containing the
complete anchor/lease identity and prune operation for each member, records the
exact count/digest result, and cannot delete an unexpired lease. Replay verifies
the tombstones plus source-row absence. Update is always
rejected; delete triggers require the matching active journal owner. A snapshot
page using an expired lease returns `TIMELINE_SNAPSHOT_EXPIRED`; a subscription
using an expired/pruned empty handoff cursor emits one
`requiresFullRefetch = true` control frame and closes without replay. Cleanup,
cursor lookup, and replay races are linearized by the same DbWriter/read
snapshot. Fixtures cover 0/1/64/65 per-run and 4096/4097 global anchors,
expiry boundary, prune-before/after lookup, restart, repeated stale cursor, and
prove deterministic full-refetch with no synthetic event or cross-run eviction.

The production Timeline load begins with the authorized
`runtimeTimelineSnapshot` query, not an empty subscription. `first` must equal
256. On the first page, null `snapshotCursor` and `afterCursor` cause one read
transaction to freeze the run's current upper `event_sequence`, count, and
ordered digest into opaque `timeline_snapshot_v1` cursor bytes and return the
newest page at or below that bound plus the separate resolvable
`timeline_cursor_v1` `handoffCursor` for that upper bound. The response presents that page in ascending
global-sequence order and returns `nextPageCursor` only when an older page
exists. Later pages are explicit `load_older` requests and must present that
exact snapshot cursor and the prior `nextPageCursor`; they cannot advance the
upper bound and must repeat the byte-identical handoff cursor. Each page is
capped at 256 rows and 1 MiB. `historyCompleteness` is `complete` with null
`legacyGap` only when no gap row exists. Otherwise it is `legacy_gap` and
`legacyGap` is the exact `runtime_timeline_legacy_gap_v1` aggregate of source
row counts/digests plus the nullable earliest reliable cursor. Reaching the
oldest reliable page sets `hasMore = false`; the client renders the fixed
non-retry row `Earlier activity unavailable` and never requests across or
invents rows for that gap. Completed runs therefore
render their newest durable history without requiring a live event, while the
entire retained history is never a prerequisite for first publication.
`upperEventSequence` is the canonical unsigned-decimal frozen bound and
`snapshotDigest` is the 64-hex ordered digest; both are repeated byte-identically
on every page and are included in every page cursor MAC.

Immediately after validating the first page, Swift subscribes with its returned
`handoffCursor` through the additive `durableAfterCursor` argument on
`runtimeStatusChanged`; it does not wait for older pages. The server registers
the bounded live receiver before reading durable rows strictly after the cursor,
freezes a replay high-water, emits those rows in sequence order, then drains
buffered live rows above that high-water. Client and server dedupe only by
`(durableEventSequence, runtime_event_id, canonicalEventDigest)`. Equal sequence
and ID with another digest is mutation/corruption; a repeated exact triple is a
duplicate; any non-successor sequence after overlap removal is a gap. Receiver
overflow, a cursor below retained history, or any sequence gap emits the
existing `requiresFullRefetch = true`; the publication owner increments only a
Timeline generation and refetches the first snapshot page plus subscription.
It does not reset the run load, topology, selection, or focus state. The legacy
`replayCursor`/`sequenceCursor` fields remain byte-compatible for old clients,
but the updated app never treats them as durable history authority. Snapshot
and subscription authorization use the same live principal/run scope, and
revoke/disable/re-scope closes the stream before another row is emitted.

`TimelineLoadStateV1` is the closed tagged union
`idle | loading_initial(timelineGeneration) |
ready(timelineGeneration, snapshotCursor, handoffCursor, nextPageCursor?,
historyCompleteness, legacyGap?) |
loading_older(timelineGeneration, snapshotCursor, handoffCursor,
nextPageCursor) |
failed_initial(timelineGeneration, failureCode, requestDigest) |
failed_retaining_rows(timelineGeneration, failureCode,
failedRequest = older(snapshotCursor, handoffCursor, nextPageCursor)) |
gap_refetch(timelineGeneration)`. One Timeline owner retains at most 512 events
and 8 MiB across snapshot pages and the live stream. The live receiver itself is
capped at 256 events and 1 MiB. Before accepting an older page or live row that
would exceed either client cap, it evicts the farthest non-visible historical
page, retaining that page's resumable cursor; it never evicts the visible page
or an undelivered live row. `load_older` is disabled when no non-visible page can
be evicted without violating those rules. Old clients that omit
`durableAfterCursor` retain their pre-existing live-only behavior unchanged.

`TimelineFailureCodeV1` is closed to the exact values
`timeline_run_id_invalid`, `timeline_cursor_mode_invalid`,
`timeline_snapshot_cursor_invalid`, `timeline_page_cursor_invalid`,
`timeline_durable_cursor_invalid`, `timeline_cursor_run_mismatch`,
`timeline_page_size_invalid`, `timeline_run_unauthorized`,
`timeline_snapshot_expired`,
`timeline_snapshot_anchor_persistence_failed`, plus `transport_unavailable`,
`response_schema_mismatch`, `page_byte_limit_exceeded`, and
`subscription_overflow`. `timeline_snapshot_expired` and a valid cursor below retention
start one new `gap_refetch` generation; they never reuse the old snapshot.
`transport_unavailable` and `timeline_snapshot_anchor_persistence_failed` expose the
generation-qualified retry target defined below. Authorization, malformed
identity/cursor/mode/page-size, and response-schema failures retain visible rows
but expose no automatic retry. No unknown error is reduced to empty data.

Presentation is exhaustive by phase:

| Timeline phase/result | Rows retained | Legal presentation/control |
|---|---:|---|
| `loading_initial` | 0 | `timeline_initial_loading/loading_timeline_initial`, disabled and focusable |
| retryable `failed_initial` | 0 | `timeline_initial_failure/retry_timeline_initial` for the exact generation/code/request digest |
| non-retryable `failed_initial` | 0 | `timeline_initial_failure/timeline_initial_failure`, readable with no action |
| `ready` with older cursor | yes | `timeline_load_older/load_older` |
| `loading_older` | yes | `timeline_load_older/loading_older`, disabled |
| retryable older failure | yes | `timeline_load_older/retry_load_older` for the exact cursor tuple |
| non-retryable older failure | yes | fixed inline error row, no target mutation |
| `ready` with `legacy_gap` | yes or 0 | one `timeline_legacy_gap/legacy_gap_row`, exact text `Earlier activity unavailable`, no retry/action |
| `gap_refetch` | prior rows remain read-only until atomic replacement | `timeline_initial_loading/loading_timeline_initial`; no old cursor action |

Retryable initial codes are exactly `transport_unavailable` and
`timeline_snapshot_anchor_persistence_failed`; every other initial code is
non-retryable until a new run/navigation load. Activating initial retry creates
one new `timeline_generation`, captures the current run/load/window tuple, and
never mutates the failed generation. Initial/older callbacks with another
generation, request digest, snapshot/page cursor, run, load generation, window,
or mount token are byte-for-byte no-ops. The legacy-gap row has a deterministic
subject derived from `(run_id, legacy_gap.sourceDigest,
earliestReliableCursor-or-empty)`, participates in Timeline keyboard order,
and exposes label/value `Earlier activity unavailable` with Help
`Runtime activity before durable Timeline history is unavailable.`; it is not a
failure or retry control.

Timeline lane inventory is derived from occurrence truth and associated event
truth, not from a permanently synthesized run lane:

| Occurrences | Unassociated events | Exposed lanes / initial selection |
|---|---|---|
| none | none | no lane; loaded `empty` |
| one or more, including zero-event occurrences | one or more | one lane per occurrence plus `Run events`; select the normal occurrence default |
| one or more, including zero-event occurrences | none | one lane per occurrence only; select the normal occurrence default and show `No events` inside a selected zero-event lane |
| none | one or more | `Run events` only; select it |

Selecting an occurrence filters only its lane; selecting `Run events` exposes
only unassociated events. GraphQL returns the exact non-null lane tuple on each
event. `P031RunDetailReadModel` builds its required lane inventory from every
topology occurrence plus those event tuples: it therefore creates an occurrence
lane even when its event list is empty and creates the run-events lane iff at
least one authorized unassociated event exists. Fixtures prove all four rows,
occurrence creation/removal, an occurrence whose event count remains zero, and
the first/last unassociated event transition without inventing an empty run
lane.

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

The shipped Timeline documents are exact and share one source fragment. The
fragment includes every pre-proposal field plus the additive durable identity;
tests byte-compare these complete operation bytes, not only field presence:

```graphql
fragment P031RuntimeTimelineEventFields on GqlRuntimeEvent {
  id runId stageId agentId provider eventKind title detail surfaceLabel
  sessionGenerationId timestamp rawDetail rawDetailBytes rawDetailTruncated
  rawDetailHandle rawDetailDigest fullRawAvailable detailDigest detailCharCount
  chunkCount isStreaming isTerminal stateLabel sequenceCursor
  projectionGeneration gapDetected requiresFullRefetch
  agentExecutionId taskOccurrenceId taskOccurrenceSequence presentationRowId
  timelineLaneId timelineLaneKind timelineIdentityState timelineLaneEventOrdinal
  presentationProviderIdentity { provider model effort identityDigest }
  durableEventSequence canonicalEventDigest durableTimelineCursor
}

query P031RuntimeTimelineSnapshot(
  $runId: ID!, $snapshotCursor: String, $afterCursor: String, $first: Int!
) {
  runtimeTimelineSnapshot(
    runId: $runId, snapshotCursor: $snapshotCursor,
    afterCursor: $afterCursor, first: $first
  ) {
    schemaVersion snapshotCursor upperEventSequence snapshotDigest
    handoffCursor nextPageCursor hasMore
    historyCompleteness
    legacyGap {
      schemaVersion reason sourceRowCount sourceDigest earliestReliableCursor
    }
    events { ...P031RuntimeTimelineEventFields }
  }
}

subscription P031RuntimeStatusChanged(
  $runId: ID!, $durableAfterCursor: String!
) {
  runtimeStatusChanged(
    runId: $runId, durableAfterCursor: $durableAfterCursor
  ) { ...P031RuntimeTimelineEventFields }
}
```

The initial variables are exactly `runId`, null `snapshotCursor`, null
`afterCursor`, and `first = 256`. Historical-page variables repeat the returned
snapshot cursor and prior `nextPageCursor`; every page repeats the same
`handoffCursor`, and subscription variables are exactly that run ID and first
page's handoff cursor. Unknown/extra variables, a nullable durable
subscription cursor, `first != 256`, a page cursor without its snapshot cursor,
or mixing durable and legacy cursors is rejected. A retained legacy-client
fixture executes the old `runtimeStatusChanged(runId:, replayCursor:)` document
unchanged against the additive schema.

`P031TimelineGraphQLErrorCodeV1` mirrors the complete server code set above.
The sole generated mapping removes no words: each uppercase server token maps
to its exact full lowercase token, for example
`TIMELINE_RUN_ID_INVALID -> timeline_run_id_invalid` and
`TIMELINE_SNAPSHOT_ANCHOR_PERSISTENCE_FAILED ->
timeline_snapshot_anchor_persistence_failed`; generated exhaustiveness covers
all ten pairs. The HTTP decoder requires schema/code/requestId and the
WebSocket decoder requires schema/code plus its outer operation ID before
mapping a response into `TimelineLoadStateV1`; neither drops `extensions` into
a generic message. Missing
or unknown schema/code, data together with a terminal request error, malformed
error JSON, missing HTTP request ID, missing/mismatched WebSocket operation ID,
extra extension key, or an unlisted future code maps locally to
`response_schema_mismatch` and never retries automatically. Decoder fixtures
exercise every code byte-for-byte over both transports, including malformed
`runId`, and prove a rejected subscription leaves no receiver task.

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

Generated run-report schemas and every non-P079 artifact path remain unchanged.
This proposal does not add a report candidate, truth epoch, artifact lease, or
canonical-report rewrite. The one explicit filesystem/materialization exception
is the P079 repair/fallback redesign above: provider-authored candidate bytes are
written only to operation staging, and runtime-owned history/canonical activation
publishes accepted bytes at the same existing logical/canonical destinations.
It removes the P079 canonical-path write grant but does not alter Codex
`danger-full-access`, ordinary worktree/output grants, Steward output paths, or
adapter containment for any other lane. Accepted model/effort truth is durable
database state read through GraphQL; it is not injected into provider-authored
files. The structural gate rejects any new `run-v2://`, report variant,
`ProviderFilesystemProfileV1`, non-P079 materializer/grant change, or MCP
`structuredContent` implementation in this slice, while positively requiring
the named P079 staging and activation path.

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

The shipped `P031GraphQLDocumentSet` has proposal-owned properties
`providerExecutionTruthSchemaProbe`, `runDetail`, `runStageTopologyPage`,
`occurrenceExecutionAttemptPage`, `runtimeTimelineSnapshot`,
`runtimeStatusChanged`, `timelineRawDetail`, and `daemonStatus`; no test-only
literal substitutes for any of them. `P031GraphQLRunReadClient` exposes the
matching probe, paged topology/attempt, paged Timeline snapshot,
subscription-handoff, and raw-detail methods,
and `P031RunsHomeViewModel` calls those production methods. Complete document
snapshots and a call-site inventory prove every property is transported and
decoded.

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
  invalidatingOptionSnapshotRevision configurationFailureCode
  configurationCancellationCode postConfigurationOutcomeCode
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
      invalidatingOptionSnapshotRevision configurationFailureCode
      configurationCancellationCode postConfigurationOutcomeCode
    }
  }
}

# P031RunDetail selects no stages.executions, runStageTopology occurrence,
# transition, or executionAttempts arrays. Topology and attempts use only the
# snapshot-bound paged operations below.

# Added under activeAgentExecutions
taskOccurrenceId taskOccurrenceSequence presentationRowId
presentationProviderIdentity { provider model effort identityDigest }
providerExecutionTruth { ...P031ProviderExecutionTruthFields }

query P031RunStageTopologyPage(
  $runId: ID!, $topologySnapshotCursor: String,
  $afterCursor: String, $first: Int!
) {
  runStageTopologyPage(
    runId: $runId, topologySnapshotCursor: $topologySnapshotCursor,
    afterCursor: $afterCursor, first: $first
  ) {
    schemaVersion topologySnapshotCursor topologySnapshotDigest
    pageState nextCursor
    stageHeaders {
      stageId frozenWorkflowOrdinal legacyOrderUnverified stageDigest
    }
    entries {
      entryOrdinal kind stageId
      occurrence {
        presentationRowId compiledTaskId taskOccurrenceId occurrenceSequence
        occurrencePosition sourceKind sourceStableId frozenTaskOrdinal
        humanSourceOrdinal activeExecutionId executionAssociationState
        legacyAmbiguousExecutionCount agentId agentTitle taskName status
        provider model effort executionCount
        presentationProviderIdentity { provider model effort identityDigest }
        providerExecutionTruth { ...P031ProviderExecutionTruthFields }
      }
      transition { transitionId transitionOrdinal toStageId toLabel detail }
    }
  }
}

query P031OccurrenceExecutionAttemptPage(
  $runId: ID!, $taskOccurrenceId: ID!, $topologySnapshotCursor: String!,
  $attemptAfterCursor: String, $first: Int!
) {
  occurrenceExecutionAttemptPage(
    runId: $runId, taskOccurrenceId: $taskOccurrenceId,
    topologySnapshotCursor: $topologySnapshotCursor,
    attemptAfterCursor: $attemptAfterCursor, first: $first
  ) {
    schemaVersion topologySnapshotCursor topologySnapshotDigest
    pageState nextAttemptCursor
    attempts {
      id status startedAt completedAt
      presentationProviderIdentity { provider model effort identityDigest }
      providerExecutionTruth { ...P031ProviderExecutionTruthFields }
    }
  }
}
```

The retained compatibility field
`RunStageTopologyOccurrence.executionAttempts: [GqlAgentExecution!]!` remains
ordered `started_at DESC, id DESC`, but its resolver rejects a result over 32
rows or 256 KiB with exact `topology_graphql_error_v1` code
`TOPOLOGY_LEGACY_QUERY_OVER_LIMIT`; it never materializes or truncates an
unbounded array. The same selected-field lookahead rule applies to legacy
`stages.executions`: its cap is evaluated only when that exact nested field is
present in the incoming operation. A query that does not select either array
does not count/materialize attempts and cannot fail because 33 or more attempts
exist. The updated app never selects either field and uses
`P031OccurrenceExecutionAttemptPage` instead.

`P031RunStageTopologyPage` has one total cursor state machine over a canonical
merged entry order: stage headers are `(frozenWorkflowOrdinal, stageId)` and
entries are `(stageOrdinal, kindOrdinal[occurrence=0,transition=1],
source/transition ordinal, stable ID)`. Initial request is exactly null snapshot
and null `afterCursor`; it uses `first = 128` and freezes all stage metadata,
occurrences, transitions, source revision/counts, and ordered digest into opaque
`topology_snapshot_v1` bytes. A response with `pageState = more` requires one
non-null `nextCursor`; `exhausted` requires null. Continue requires the exact
snapshot plus prior non-null cursor and returns the next merged page; requesting
again after exhausted, a cursor in initial mode, null cursor in continue mode,
or any cross-snapshot cursor is rejected before a read. Every page repeats the
same snapshot digest and complete bounded stage-header array, and each entry has
exactly one of occurrence/transition non-null according to `kind`. Each page is
at most 128 merged entries, 256 stage headers, and 1 MiB decoded. The publication
owner accepts at most 1,024 occurrences, 4,096 transitions, 256 stages, and
8 MiB for one snapshot. A source that exceeds any total cap fails before partial publication
as `TopologyFailureCodeV1.projection_size_limit_exceeded`; the server error is
the exact `topology_graphql_error_v1` code
`TOPOLOGY_PROJECTION_SIZE_LIMIT_EXCEEDED`. Its recovery disposition is
`limit_only`: the UI exposes the noninteractive summary but no restart,
replacement, report, or retry action. Snapshot mismatch/expiry, duplicate/missing page
items, count/digest drift, and a byte cap violation fail the same generation
without mixing pages.

`P031OccurrenceExecutionAttemptPage` uses exactly `first = 32`, a 256-KiB
response cap, the same topology snapshot/digest, exact occurrence ownership, and
total order `started_at DESC, id DESC`. Its request/response state machine is the
same exact `initial(null cursor) -> more(non-null cursor) -> exhausted(null)`
algebra; continue after exhausted or cursor/snapshot mismatch is rejected.
Swift retains at most 128 attempts and 1 MiB
per selected occurrence; loading a fifth full page evicts the farthest
non-visible page while preserving its cursor for a later explicit reload. It
never truncates the visible page or folds attempts into aggregate execution
truth. The old monolithic `runStageTopology` resolver is byte-compatible only
for selected results within 1,024 occurrences, 4,096 transitions, 256 stages,
and 8 MiB. The 32-attempt/256-KiB per-occurrence cap is additionally evaluated
only for an `executionAttempts` subfield actually selected in that operation;
unselected execution fields trigger no attempt query/count. Above an applicable limit it returns
`TOPOLOGY_LEGACY_QUERY_OVER_LIMIT` before constructing the response. Provider-
free tests cover exact-boundary and plus-one rows/bytes, page reorder/duplicate,
snapshot races, eviction/reload, and both legacy error paths.
`P031GraphQLDocumentSet.runtimeTimelineSnapshot` with operation
`P031RuntimeTimelineSnapshot` selects/decodes the snapshot schema version,
distinct snapshot/handoff/page cursors, `hasMore`, and the complete event fields.
`P031RuntimeTimelineEventReadModel` and the shipped
`P031GraphQLDocumentSet.runtimeStatusChanged` property with operation
`P031RuntimeStatusChanged` select/decode `agentExecutionId`,
`taskOccurrenceId`, `taskOccurrenceSequence`, `presentationRowId`,
`timelineLaneId`, `timelineLaneKind`, `timelineIdentityState`,
`timelineLaneEventOrdinal`, presentation identity, `durableEventSequence`,
`canonicalEventDigest`, `durableTimelineCursor`, `projectionGeneration`, and
`gapDetected`; the subscription passes
the first page's handoff cursor as `durableAfterCursor`. The
shipped `P031GraphQLDocumentSet.timelineRawDetail` property with operation
`P031TimelineRawDetail` and `P031TimelineRawDetailReadModel` select/decode the
same identity/lane/ordinal fields plus `timelineEventId`. Its document also selects the
existing status/raw/error fields so the nullability matrix above is decoded in
one response. No production document reconstructs identity by agent ID.

The exact shipped DTO changes are:

- `P031StageAgentExecutionReadModel` adds occurrence ID/sequence,
  presentation-row ID, frozen presentation provider identity, and non-null
  `P031ProviderExecutionTruthReadModel`;
- `P031ProviderPromptTurnReadModel` adds non-null
  `P031ProviderPromptConfigurationTruthReadModel`; its presence-aware decoder
  implements the exact new/non-Codex/legacy nullability matrix rather than
  reading the execution-level current receipt;
- `P031ActiveAgentExecutionReadModel` adds occurrence ID/sequence,
  presentation-row ID, frozen presentation provider identity, and non-null truth;
- `P031RunStageTopologyOccurrenceReadModel` adds every exact topology field,
  including non-null `humanSourceOrdinal` and presentation provider identity;
  attempts live in the separately
  paged `P031OccurrenceExecutionAttemptPageReadModel` rather than an unbounded
  nested array;
- `P031RunStageTopologyReadModel` adds frozen workflow ordinal and legacy-order
  state, while `P031RunStageTopologyTransitionReadModel` adds transition ID and
  ordinal; `P031RunStageTopologyPageReadModel` owns the immutable snapshot and
  sole merged page cursor/state;
- `P031RuntimeTimelineSnapshotReadModel`,
  `P031RuntimeTimelineEventReadModel`, and
  `P031TimelineRawDetailReadModel` add the exact event identity, lane tuple, and
  per-lane event ordinal, frozen presentation identity when occurrence-bound,
  64-bit-safe durable sequence, canonical digest, projection generation,
  gap flag, and durable cursor;
  the latter preserves all six existing status cases and their closed
  nullability; and
- `P031RunDetailReadModel` selects no stage execution/topology/attempt arrays,
  retains its bounded non-topology arrays, joins only
  validated paged topology/attempt objects, stores the deduplicated durable
  Timeline snapshot plus subscription
  tail, and deterministically derives the conditional lane inventory from
  topology occurrences plus authorized event tuples; there is no parallel
  test-only read model.

Topology decode is the one deliberate field-level recovery boundary.
Production `P031RunDetailEnvelopeReadModel` decodes only bounded non-topology
run-detail fields with the normal strict presence-aware decoder. Independently,
`P031TopologyPageEnvelopeReadModel` retains each page as duplicate-key-rejected
raw JSON, validates one snapshot cursor/digest plus contiguous merged entry
ordinals and repeated stage-header bytes, and only after the complete snapshot
is present reduces it to `P031TopologyDecodeResultV1 =
available(P031RunStageTopologyReadModel) | unavailable(TopologyFailureCodeV1)`:
missing initial page or explicit null is `projection_absent`; a missing field, unknown enum,
wrong scalar/list/nullability, or malformed topology object is
`projection_schema_invalid`; a digest/owner/source join failure is
`projection_source_invalid`; and duplicate/negative/non-contiguous ordinals or
an ordering invariant failure is `projection_order_invalid`. A structurally
valid projection whose frozen stage or task label fails `BoundedHumanLabelV1`
is `frozen_input_invalid`, not a projection/daemon failure. Crossing a declared aggregate row or
byte cap is `projection_size_limit_exceeded`, not a schema failure or partial
projection. The raw error text
is sanitized and not stored in UI state. A failure anywhere outside that exact
subtree remains `response_schema_mismatch` and fails the load. Thus malformed
topology preserves the separately loaded authorized run detail and reaches the
recoverable `topology_unavailable` state without weakening strict decoding for
provider truth, timeline, approvals, or reports.

Occurrence position is owned only by the topology occurrence projection; it is
not copied onto execution/event rows where it could become stale. After decode,
`P031RunDetailReadModel` constructs mandatory
`OccurrencePresentationJoinV1(runID, presentationRowID, taskOccurrenceID,
stageID, occurrencePosition, humanSourceOrdinal,
FrozenPresentationProviderIdentityV1)` entries from topology rows.
Every v2 execution, active-agent row, and `matched_occurrence_v2` timeline event
joins by exact `(runID, presentationRowID)` and, when present, must match the
same task-occurrence ID and byte-identical presentation identity digest.
Missing, duplicate, cross-run, identity-, or position/source-mismatched joins
are typed schema failures. Historical execution effort may be null; every
occurrence-bound surface renders provider/model/effort from this frozen tuple,
never from the nullable historical execution scalar. An explicitly
authorized `unassociated_run_event` bypasses the join and must target the
run-events lane. Position changes are published by replacing this join snapshot,
never by rewriting historical events.

Raw-detail decode is deliberately conditional rather than treating all six
statuses as occurrence-shaped. The decoder first validates the complete
status/raw/error/identity nullability row, then applies exactly one branch:

| Raw-detail row | Occurrence-presentation action |
|---|---|
| `available`, `stale`, `unavailable`, or `digest_mismatch` with non-null identity tuple and `matched_occurrence_v2` | Require the exact join and reject any tuple mismatch |
| Authorized `unassociated_run_event` with a non-null run-events lane | Bypass occurrence join and preserve the run-events lane |
| `missing` or `unauthorized` | Require execution, occurrence, presentation-row, event, and lane identity fields all null; do not construct or attempt a join |

No missing or unauthorized response may recover identity from the request
handle, cached row, selected occurrence, or prior response. Conversely, a
status that legally carries matched identity may not silently degrade to a null
join. Production-document/decoder fixtures execute all six rows and both legal
identity branches; mutation negatives swap each status, identity state, lane,
and nullable identity field.

Every DTO declares every `CodingKey` and uses a custom decoder that distinguishes
`container.contains(key) == false` (typed schema mismatch) from explicit null
(valid state), except that the outer envelope maps the topology subtree through
the closed field-level result above. Checked-in GraphQL and Swift fixtures cover historical Codex,
non-Codex, pre-session configuration failure, mediation, both P079 repairs, all
three P086 continuation modes, empty legacy turns, same-agent occurrences, and
schema mismatch. A document snapshot test byte-compares the complete production
schema probe, `P031RunDetail`, `P031RuntimeTimelineSnapshot`,
`P031RuntimeStatusChanged`, and `P031TimelineRawDetail` operation strings from
those exact properties; a decoder test fails if
any required selected field is absent.

### Closed public values and nullability

Rust enums, GraphQL enums, and Swift enums are generated or byte-compared from
these closed wire domains:

| Domain | Exact JSON values |
|---|---|
| configuration state | `configuring`, `configured`, `invalidated_after_acceptance`, `failed_before_prompt`, `cancelled_before_configuration`, `configured_terminated_before_prompt`, `legacy_unverified` |
| configuration evidence | `pending`, `receipt_available`, `readiness_available`, `invalidated`, `failure_available`, `cancellation_available`, `not_applicable`, `legacy_unverified` |
| configuration failure code | `model_unavailable`, `model_not_accepted`, `effort_unavailable`, `effort_not_accepted`, `acceptance_persistence_failed`, `provider_start_failed`, `provider_process_identity_unverified`, `configuration_deadline_elapsed`, `resume_unsupported`, `resume_configuration_unavailable`, `configuration_evidence_invalid` |
| configuration cancellation code | `cancelled_before_configuration` |
| post-configuration outcome code | `configured_cancelled_before_prompt`, `configured_deadline_before_prompt`, `configured_transport_lost_before_dispatch`, `configured_superseded_for_resurrection`, `provider_ready_cancelled_before_prompt`, `provider_ready_deadline_before_prompt`, `provider_ready_transport_lost_before_dispatch`, `provider_ready_superseded_for_resurrection` |
| effective provider capability | `codex_exact_pair_v1`, `not_applicable_v1`, `legacy_best_effort_v0` |
| acceptance source | `fresh_negotiation`, `reused_session_generation`, `attached_session_reverification` |
| prompt kind | `original`, `code_writer_completion_repair`, `output_contract_repair`, `work_continuation_live_handle`, `work_continuation_resurrection`, `work_continuation_output_only`, `steward_analysis` |
| prompt owner kind | `invoke_agent`, `p017_mediation`, `p058_escalation`, `p079_repair`, `p079_fallback_child`, `p086_continuation`, `steward_agent_lane` |
| configuration owner kind | `agent_execution`, `p017_mediation_execution`, `p079_repair_attempt`, `p086_continuation`, `steward_agent_lane` |
| prompt dispatch state | `not_started`, `dispatch_pending`, `prompt_sent`, `dispatch_unknown` |
| prompt-turn failure code | `configuration_failed`, `prompt_preparation_failed`, `owner_cancelled_before_prompt`, `owner_superseded_before_prompt`, `provider_generation_interrupted_before_prompt`, `prompt_transport_failed`, `prompt_delivery_unknown`, `provider_runtime_failed_after_prompt`, `provider_runtime_timeout_after_prompt`, `provider_generation_interrupted_after_prompt`, `legacy_authority_unverifiable` |
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
- `configured` and `configured_terminated_before_prompt` require accepted
  model/effort, wire pair, acceptance source,
  generation/session binding, receipt digest, verified time, and
  `receipt_available` together;
- `invalidated_after_acceptance` preserves the historical accepted pair and
  receipt but requires evidence `invalidated`, the invalidating option-snapshot
  revision/digest, non-null invalidated time/revision readback, and UI copy that
  runtime identity is unknown; it is never rendered as actual;
- every non-invalidated state requires invalidated time/revision readback null;
- `configuring`, `failed_before_prompt`,
  `cancelled_before_configuration`, and `legacy_unverified` require all
  accepted/source/verified fields null;
- non-Codex execution requires configuration state, accepted wire pair,
  acceptance source, and configuration receipt null; its evidence is `pending`,
  `readiness_available`, `failure_available`, or `cancellation_available` from
  the generated matrix. Only a planned shell without an execution uses
  `not_applicable`;
- `failure_available` requires `failed_before_prompt`, a null receipt and
  cancellation pointer, a matching immutable configuration-failure row/code,
  and a `not_started` prompt turn;
- `cancellation_available` requires `cancelled_before_configuration`, null accepted
  values and receipt/failure pointers, a matching immutable pre-acceptance
  cancellation row/code, and a `not_started` prompt turn;
- `configured_terminated_before_prompt` requires `receipt_available`, the
  complete accepted pair/receipt, null failure and cancellation pointers,
  exactly one immutable receipt-qualified
  `provider_post_configuration_outcomes_v1` row/code,
  and a `not_started` prompt turn;
- provider-neutral/legacy terminality after readiness keeps configuration state
  and accepted values null, retains `readiness_available`, requires exactly one
  readiness-qualified `provider_ready_*` row matching the cause, and keeps the
  source prompt turn `not_started`;
- every execution-shaped DTO has a non-null prompt summary and non-null turn
  array, including an empty legacy array; topology with no execution has a
  non-null `ProviderExecutionTruth` shell whose execution/requested/accepted
  scalars are null plus a non-null `not_started` summary, with configuration
  evidence `pending` for planned exact Codex, `not_applicable` for planned
  non-Codex, or `legacy_unverified` for a legacy row;
- every prompt turn has a non-null configuration-truth shell. New exact-pair
  turns require owner kind/ID and attempt index; `receipt_available` and
  `invalidated` additionally require generation and the complete accepted pair;
  `failure_available`/`cancellation_available` require their one matching code
  and evidence row with no receipt; a post-configuration outcome instead
  retains the receipt and exposes only its typed outcome code;
  non-Codex uses the matching pending/readiness/failure/cancellation evidence,
  while only a planned non-exact shell uses `not_applicable` and only migrated
  unlinked turns may use `legacy_unverified` with nullable owner/attempt/
  generation fields;
- every prompt turn's receipt-link state is null when no receipt links to that
  turn; the non-null link summary counts linked and unlinked receipts, sums to
  total receipt count, and uses the frozen worst-state order;
- receipt JSON always emits every declared nullable key as explicit null;
  GraphQL omits none of the declared fields, and Swift treats omission as a
  schema mismatch; and
- every runtime event has non-null lane ID/kind/identity state. Raw-detail
  identity nullability is determined only by the exact six-status matrix above,
  never by decoder convenience.

Swift declares exhaustive `ProviderConfigurationFailureCodeV1`,
`ProviderConfigurationCancellationCodeV1`, and
`ProviderPostConfigurationOutcomeCodeV1` enums generated or byte-compared from
the same table. Their decoders have no `.unknown`, raw-string display fallback,
or `String` storage. `ProviderFailurePresentationV1` carries the closed phase
`provider_configuration | prompt_dispatch | provider_runtime` separately from
those codes; no pre-prompt configuration row is represented by an
`AcpRuntimeReceipt` or its `failure_phase` field.

The single schema fixture enumerates every legal state/nullability row and
mutation-negatives for one missing key, one unknown enum, one half-populated
accepted pair, configured-without-receipt, legacy-with-accepted-values, and
non-Codex configuration leakage.

### Lockstep daemon schema

GraphQL rejects a document containing unknown fields; an old daemon does not
return those fields as `nil`. The updated app therefore requires lockstep
replacement of the bundled daemon rather than issuing a reduced legacy run
detail query.

The probe SDL is `providerExecutionTruthSchemaVersion: Int!` on `QueryRoot`.
The only probe document is the shipped
`P031GraphQLDocumentSet.providerExecutionTruthSchemaProbe` property with exact
bytes
`query ProviderExecutionTruthSchemaProbe { providerExecutionTruthSchemaVersion }`.
`P031GraphQLRunReadClient.probeProviderExecutionTruthSchema()` sends that
property through `P031URLSessionGraphQLReadTransport`, decodes the presence-
aware `P031ProviderExecutionTruthSchemaProbeReadModel`, and accepts only
`data.providerExecutionTruthSchemaVersion == 1` with no GraphQL errors. The
document-set snapshot, transport spy, decoder fixture, and replacement-flow
test all use this production property; a recursive source scan rejects a second
probe literal or a test-only transport. Handling is frozen:

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
| Superseded for resurrection | Response-verified pair; source output-only turn stayed `not_started` | `Superseded for resurrection: Codex - GPT-5.6 Terra - High - Configuration accepted - No prompt sent` |
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
inconsistent. It renders exactly `Runtime identity unavailable`; its Help and
copy text are exactly
`Runtime identity unavailable - Configured execution is missing accepted model or effort.`
and it must be caught by the gate. It must not fall back to planned truth.

`legacy_ambiguous` is a legal formatter input, not a schema failure. It renders
exactly `Runtime identity unavailable - Multiple legacy executions`; its Help
and copy text are exactly
`Runtime identity unavailable - Multiple legacy executions: <count>.`, where
`<count>` is the bounded candidate count in canonical non-negative ASCII
decimal. It has no accepted/requested runtime pair. Planned task identity may
remain on its separate planned line. It never selects one legacy execution. A
configured execution that fails before permit is likewise the legal `Start
failed after configuration` row above rather than an impossible tuple.

For every other legal row in the Codex, legacy, and provider-neutral matrices,
`helpText` and `copyText` are byte-equal to that row's exact `Operator copy`.
For every illegal tuple not covered by the configured-missing-acceptance case,
the visual text is exactly `Runtime identity unavailable` and Help/copy are
exactly
`Runtime identity unavailable - Provider execution truth is inconsistent.`
There is no caller-supplied diagnostic suffix. The formatter's accessibility
identity is the separately derived `accessibilityIdentity` defined below;
occurrence and control labels wrap that safe value using the exact templates
below. Help and copy remain byte-equal and preserve a bounded escaped diagnostic
representation, but
no unknown raw segment is promoted into spoken output.

Legacy generic Codex status is also closed rather than assembled from an
unspecified prefix:

| Legacy state | Exact operator copy |
|---|---|
| planned | `Planned: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| starting | `Starting: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| running after prompt | `Running: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| completed after prompt | `Completed: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| failed after prompt | `Failed: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| cancelled after prompt | `Cancelled: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Unverified` |
| delivery unknown | `Prompt delivery unknown: Codex - GPT-5.6 (variant unspecified)<legacyEffortSegment> - Do not retry automatically - Unverified` |

`<legacyEffortSegment>` is derived only from the frozen requested effort. It is
exactly ` - Low`, ` - Medium`, ` - High`, ` - Extra High`, ` - Max`, or
` - Ultra` for the six known values, uses ` - ` plus the bounded diagnostic
mapping for an unknown non-empty value, and is the empty string when historical
effort is absent. The formatter never supplies `High` merely because a legacy
profile omitted effort. Generated goldens cross every state with all six known,
all supported frozen provider/model variants, unknown bounded values, and null,
then byte-compare Overview, Stages, active-agent rows, Timeline occurrence
headers, popover, copy, accessibility, and Run Inspector output. The nullable
historical execution effort is never the presentation source.

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
| Superseded for resurrection | `Superseded for resurrection: Claude - opus - High - No prompt sent - Acceptance unverified` |
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
`Junie`; an unknown non-empty provider uses
`BoundedDiagnosticIdentitySegmentV1(provider, raw)` in full
output. Only the four explicit Codex model mappings below (three named variants
plus the generic model) change model spelling. Every other
provider model, including raw `opus`, uses
`BoundedDiagnosticIdentitySegmentV1(model, raw)`; the formatter never
title-cases, trims, aliases, or locale-folds an unknown/raw model.

| Raw model | Display value |
|---|---|
| `gpt-5.6-sol` | `GPT-5.6 Sol` |
| `gpt-5.6-terra` | `GPT-5.6 Terra` |
| `gpt-5.6-luna` | `GPT-5.6 Luna` |
| `gpt-5.6` | `GPT-5.6 (variant unspecified)` |
| unknown non-empty model | `BoundedDiagnosticIdentitySegmentV1(model, raw)` |

| Raw effort | Display value |
|---|---|
| `low` | `Low` |
| `medium` | `Medium` |
| `high` | `High` |
| `xhigh` | `Extra High` |
| `max` | `Max` |
| `ultra` | `Ultra` |
| unknown non-empty effort | `BoundedDiagnosticIdentitySegmentV1(effort, raw)` |

`BoundedDiagnosticIdentitySegmentV1(kind, rawBytes)` has the closed `kind`
`provider | model | effort`. Valid literal display requires valid UTF-8, at most
256 Unicode scalar values, and at most 1024 input bytes. It preserves every
printable scalar except `\` and `"`; those become `\\` and `\"`. Every C0/C1
control, DEL, line/paragraph separator, and bidi control (`U+061C`,
`U+200E..U+200F`, `U+202A..U+202E`, `U+2066..U+2069`) becomes exactly
`\u{HEX}`, where `HEX` is uppercase hexadecimal padded to at least four digits
and has no redundant leading zero after four digits. The escaped result may not
exceed 2048 UTF-8 bytes. Invalid UTF-8 or any input/output bound violation
instead renders exactly `Custom <kind> sha256:<64 lowercase hex>` using
SHA-256 over `UTF8("chainworks.diagnostic_identity_segment.v1") || 0x00 ||
UTF8(kind) || 0x00 || rawBytes`. Raw unknown bytes remain available only in the
already-authorized backend evidence/readback and are never injected directly
into visual text, Help, copy, popover, logs, or accessibility. Swift and Rust
consume one generated forbidden-scalar table and checked hash vectors.

The same formatter is used by:

- current/previous stage occurrence rows in Overview;
- the Stages topology surface;
- active-agent readback rows;
- Run Inspector summary, execution-attempt rows, and identity detail popovers;
- Timeline agent headers whenever execution identity is shown; and
- Help, copy, and accessibility labels derived from those rows.

Run Inspector deletes its separate truncated planned-model formatter. It uses
the same accepted/requested state input and `fullIdentity`/`compactIdentity`
result as Overview and Stages, including effort, verification state, prompt
truth, and the bounded diagnostic unknown value in visual, Help, copy, and
popover output. Spoken output uses only the safe mapping below.

The formatter returns `fullIdentity`, `compactIdentity`, and
`accessibilityIdentity`.
`fullIdentity` always applies `BoundedDiagnosticIdentitySegmentV1` to unknown
provider/model/effort values and is the sole source for Help, copy, and the
detail popover.
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

`AccessibilitySafeIdentityV1` constructs `accessibilityIdentity` from the same
legal state row and status prefix, but substitutes each identity segment through
this closed spoken mapping before string assembly:

| Segment | Exact spoken mapping |
|---|---|
| provider `codex`, `claude`, `gemini`, `auggie`, `junie` | `Codex`, `Claude`, `Gemini`, `Auggie`, `Junie` |
| any other non-empty provider | `Custom provider` |
| model `gpt-5.6-sol`, `gpt-5.6-terra`, `gpt-5.6-luna`, `gpt-5.6` | The four exact display values above |
| any other non-empty model | `Custom model` |
| effort `low`, `medium`, `high`, `xhigh`, `max`, `ultra` | The six exact display values above |
| any other non-empty effort | `Custom effort` |
| missing optional segment | Omit the complete segment and its separator |

The state prefix and fixed verification/delivery suffixes remain byte-equal to
the applicable legal matrix row. Illegal tuples use only the fixed
`Runtime identity unavailable` accessibility value. The safe identity never
contains unknown raw bytes, a hash or hash prefix, UUID, digest, provider
session ID, opaque provider-session ref, or request ID. Visual text, Help,
copy, and the detail popover expose only the bounded diagnostic representation.
Golden fixtures include unknown values shaped as a UUID, SHA-256
digest, `psref_v1:*`, provider session ID, request ID, non-ASCII text, and
newline/tab, quote/backslash, every forbidden bidi class, 256/257 scalars,
1024/1025 bytes, invalid UTF-8 at the Rust boundary, and values whose escaping
crosses 2048 bytes. Each full/copy value is exact escaped text or the full
64-hex fallback, each compact value is hashed as specified, and every spoken
value contains only the exact safe literals above.

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
`compactIdentity`, `accessibilityIdentity`, Help, copy, and accessibility
strings; every illegal tuple
must return typed `identity_contract_invalid` rather than a best-effort label.
The oracle implements the exact Help/copy rules above, including configured
missing acceptance, bounded legacy ambiguity count, and generic illegal truth;
it does not accept free-form diagnostics from the fixture.
The gate proves each enum value appears in at least one legal golden and one
applicable mutation-negative. Visual, Help, copy, and accessibility outputs are
then byte-compared to their respective fields in the same formatter result;
accessibility is never compared to `fullIdentity` or `helpText`.

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
`selectedRowRemoved` commands. Every mounted command carries the exact target
and current mount token. Opening records the triggering `presentationRowId`;
closing requests focus restoration to that row only through a current
registration. If it
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

Every unavailable topology renders the exact state `Topology unavailable` with
the generation-qualified `topology_unavailable_summary` subject; it renders
neither the old hard-coded map nor sequential guessed nodes. For
`projection_absent`, `projection_schema_invalid`, `projection_source_invalid`,
or `projection_order_invalid`, the window-owned control is exactly
`restart_daemon_and_retry_topology`. It first invokes the production daemon
lifecycle restart coordinator and only after a new healthy daemon epoch begins
a new run-detail `loadGeneration`; it never republishes into the failed
generation. Failure to restart remains on the same summary with sanitized
daemon state. For `frozen_input_invalid`, restart is forbidden and the exact
operator guidance is `Frozen run labels are invalid. Correct the workflow and
start a replacement run.` The only action is `open_frozen_input_repair`, which
opens the frozen workflow definition and the existing replacement-run flow;
it does not mutate the original run snapshot or retry the daemon. The
`projection_size_limit_exceeded` disposition is instead `limit_only`; its exact
noninteractive guidance is `Topology exceeds the interactive limit.` It offers
no action and neither restarts the daemon nor replaces the frozen run. The
window-level recovery presenter exposes the applicable action from all seven
primary destinations and Run Inspector, including destinations without a
mounted proposal surface. A stale action or callback whose window routing ID,
request epoch, or load generation is no longer current is ignored. Hosted
negative fixtures cover every destination, all three recovery dispositions,
non-looping frozen-input guidance, stale-action rejection, and successful
restart recovery only through a new generation carrying a valid projection.

The layout golden corpus crosses each graph shape with occurrence counts
`0, 1, 2, 5, 32, 128, 256`, forward-channel counts `1, 4, 5, 8, 64`,
one-line/two-line/unknown-long identity rows, and transition
ordinals that do not match SQL input order. Every case freezes node/edge IDs,
columns, tracks, spans, edge channels, measured frames, connector endpoints,
focus order, and presentation-row IDs. A seeded stress/property corpus reaches
1,024 occurrences and 256 transitions per boundary. Required invariants are:
byte-equal output under 100 deterministic input shuffles; no frame/channel
overlap; every edge
connects its declared transition ID; every connector endpoint equals the
published frame anchor; all rows are reachable in keyboard order; and no text
or popover clips in each exact supported-window/accessibility fixture below.

The proposal adds a production Runs-window minimum rather than assuming one:
the scene/window coordinator sets `NSWindow.contentMinSize` to exactly
`920 x 620` points before publishing the first Runs surface, and the SwiftUI
root uses the same minimum so restoration cannot reopen a smaller content
area. The existing production expanded sidebar width remains `280 pt`; this
proposal does not introduce a second width constant. The responsive proof
matrix is exact: hosted snapshots run at logical content sizes `920 x 620` and
`1440 x 900`, display scale `2.0`, with sidebar
`collapsed | expanded(280 pt)`, Run Inspector `closed | open(360 pt)`, and
SwiftUI dynamic type `.large | .accessibility3`, for all 16 combinations.
At `920 x 620` the topology canvas may scroll horizontally and vertically, but
the navigation strip, window recovery presenter, Timeline older-page control,
selected row, popover, and Inspector controls must remain reachable with no
text/control overlap or clipped accessibility label. At `1440 x 900` the same
semantic target order and selection must remain byte-identical. Pixel snapshots,
accessibility frames, scroll-to-target assertions, and longest legal 256-scalar/
1024-byte labels prove containment; a plus-one label follows the typed invalid-
input path rather than resizing the viewport or occluding another control.

Each occurrence row owns its accessibility label. Stage cards contain child
accessibility elements rather than combining and swallowing occurrence labels.
Every human stage label, task name, and event title first passes
`BoundedHumanLabelV1`: valid UTF-8, 1 through 256 Unicode scalar values, at most
1024 UTF-8 bytes, and no C0/C1 control, DEL, line/paragraph separator, or bidi
control (`U+061C`, `U+200E..U+200F`, `U+202A..U+202E`,
`U+2066..U+2069`). The validator does not trim, normalize, case-fold, or replace
valid bytes. An invalid stage/task label makes the topology subtree
`frozen_input_invalid`; an absent/invalid optional event title renders the
fixed visible/spoken fallback `Event` while the bounded raw-detail lane retains
its existing authorized diagnostic bytes. Swift and Rust use the same generated
range table, and mutation fixtures cover every boundary, newline/tab, bidi
override/isolate, combining text, 256/257 scalars, and 1024/1025 bytes.
`HumanStageDiscriminatorV1` is exactly
`Stage <frozen_workflow_ordinal + 1>: <stage label>`, using the same checked
canonical integer codec below. When `legacyOrderUnverified = true`, its exact
visible companion badge is `Order unverified`, Help text is
`Stage order was reconstructed from legacy data and is unverified.`, and the
spoken discriminator is exactly
`Stage <frozen_workflow_ordinal + 1>: <stage label>, order unverified`. The badge
and spoken suffix are absent when false; no caller may silently drop or infer
the flag. Every stage-facing accessibility label and every
occurrence/event discriminator uses this value rather than a bare stage label,
so duplicate human stage titles remain distinct without exposing opaque IDs.
`OccurrenceDiscriminatorV1` is
`Planned task <human_source_ordinal + 1> in <humanStageDiscriminator>` for a planned row,
`Occurrence <occurrence_sequence + 1> for task <human_source_ordinal + 1> in <humanStageDiscriminator>`
for a durable sequenced row, or
`Legacy task <human_source_ordinal + 1> in <humanStageDiscriminator>` for a legacy row
without sequence. Ordinals use canonical ASCII base-10 with no grouping,
locale digits, sign, decimal separator, or `NumberFormatter`; Swift formats the
checked non-negative integer with the frozen POSIX-independent integer codec.
The migration never
emits more than one unsequenced legacy topology row for one source; multiple
unmatched historical executions are represented as that row's bounded
`legacyAmbiguousExecutionCount`, not duplicate spoken rows. Repeated task names
and separately materialized dynamic tasks are distinguished by unique persisted
human source ordinal; repeated occurrences of one task are distinguished by
durable occurrence sequence. No proposal-generated digest, UUID, provider-
session/request/ref ID, or abbreviated opaque identifier is introduced into a
spoken label. Existing human-authored task, stage, and event strings are
preserved after bounded-control validation even when their text happens to
match an opaque-looking grammar. The
row label is exactly
`<task name>. <occurrenceDiscriminator>. <accessibilityIdentity>. Status: <status>. Attempts: <count>.`
Here `<status>` is not free-form: `AccessibilityExecutionStatusV1` maps the
legal formatter state exactly to `Planned`, `Configuring`, `Configured`,
`Starting`, `Running`, `Completed`, `Failed`, `Cancelled`, `Superseded`,
`Prompt delivery unknown`, `Runtime identity unknown`, or
`Runtime identity unavailable`. `<count>` uses the same non-negative canonical
ASCII integer codec as the ordinals. Unknown state, negative count, or a status
not selected by the legal-state table is `identity_contract_invalid`; there is
no locale-aware or description-based fallback.
Accessibility labels are a closed subject/control matrix:

| Subject / control | Exact accessibility label |
|---|---|
| occurrence `row` | `<task name>. <occurrenceDiscriminator>. <accessibilityIdentity>. Status: <status>. Attempts: <count>.` |
| occurrence `info` | `Show full runtime identity for <task name>, <occurrenceDiscriminator>` |
| occurrence `copy` | `Copy full runtime identity for <task name>, <occurrenceDiscriminator>` |
| occurrence `popover` | `Runtime identity details for <task name>, <occurrenceDiscriminator>` |
| occurrence `close` | `Close runtime identity details` |
| timeline event `event_row` | `<event title>, <eventDiscriminator>` |
| timeline event `event_expand` | `Expand <event title>, <eventDiscriminator>` |
| timeline event `event_collapse` | `Collapse <event title>, <eventDiscriminator>` |
| timeline event `event_copy_id` | `Copy event ID for <event title>, <eventDiscriminator>` |
| timeline event `event_copy_raw` when raw source is `full` | `Copy full raw content for <event title>, <eventDiscriminator>` |
| timeline event `event_copy_raw` when raw source is `retained` | `Copy retained raw content for <event title>, <eventDiscriminator>` |
| timeline older-page `load_older` | `Load earlier activity` |
| timeline older-page `loading_older` | `Loading earlier activity` |
| timeline older-page `retry_load_older` | `Retry loading earlier activity` |
| timeline initial `loading_timeline_initial` | `Loading timeline activity` |
| timeline initial failure `timeline_initial_failure` | `Timeline activity unavailable` |
| timeline initial failure `retry_timeline_initial` | `Retry loading timeline activity` |
| timeline legacy gap `legacy_gap_row` | `Earlier activity unavailable` |
| Inspector attempt page `load_attempts_older` | `Load earlier execution attempts` |
| Inspector attempt page `loading_attempts_older` | `Loading earlier execution attempts` |
| Inspector attempt page `retry_attempts_older` | `Retry loading earlier execution attempts` |
| stage heading `stage_heading` | `<humanStageDiscriminator>` |
| run-events lane `run_events` | `Run events` |
| loading summary `loading_summary` | `Loading run activity` |
| failed summary `failed_summary` | `Run activity unavailable` |
| failed summary `retry_load` | `Retry loading run activity` |
| topology unavailable summary `topology_unavailable_summary` | `Topology unavailable` |
| topology unavailable summary `restart_daemon_and_retry_topology` | `Restart daemon and retry topology` |
| topology unavailable summary `open_frozen_input_repair` | `Open workflow and start replacement run` |
| empty summary `empty_summary` | `No run activity` |

For an occurrence timeline lane, `<laneDiscriminator>` is its exact
`OccurrenceDiscriminatorV1`; for the separate run-events lane it is exactly
`Run events`. `TimelineEventDiscriminatorV1` is exactly
`Event <timeline_lane_event_ordinal + 1> in <laneDiscriminator>`, using the same
checked canonical ASCII integer codec as occurrence ordinals. This complete
string is `<eventDiscriminator>` above, so two same-title events in one lane have
different spoken labels without exposing opaque identity. `<event title>`,
`<task name>`, `<stage label>`, and the stage label embedded in
`<humanStageDiscriminator>` are the complete existing operator-visible
strings after the same bounded-control validation as the visual control; they
are not reconstructed from an agent ID. Human-authored values are preserved
even when they happen to resemble a UUID or digest; the opaque-identity ban
applies only to provider/session/request/ref identity fields introduced or
derived by this feature. Only
occurrence controls carry an occurrence discriminator. `close`, timeline older-
page, stage heading,
run-events, loading-summary, failed-summary, topology-unavailable-summary, and empty-summary controls
intentionally do not invent one.
`TimelineEventPresentationStateV1` is the only source of event-action state. It
contains `expansion = collapsed | expanded` and
`raw_copy_source = none | full | retained`. `event_expand` is legal only for
`collapsed`; `event_collapse` is legal only for `expanded`; `event_copy_raw` is
legal only for `full` or `retained` and chooses the corresponding exact label
above. `event_copy_id` always copies only the complete event ID, while
`event_copy_raw` copies only the validated full or retained raw bytes. Reducer
tests reject stale expand/collapse targets after the state flips and
byte-compare the two clipboard payload classes so an ID can never be copied by
the raw action or vice versa.
Unknown delivery adds the exact hint `Automatic retry is blocked.` to the
occurrence row value; legacy ambiguity adds `Multiple legacy executions;
runtime identity is unavailable.` Tests assert labels, values, hints, child
order, and reducer actions byte-for-byte for every legal matrix cell and reject
every incompatible subject/control pair. Same-agent and repeated-task fixtures
assert distinct labels and action targets, including repeated event titles and
legacy backfilled ordinals. Machine uniqueness is separate:
every accessibility identifier is the canonical `PresentationTargetV1`
identifier below and may contain the full hashed row ID; it is never used as
accessibility label, value, hint, Help text, or spoken custom-action name.
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
arrive. `FrozenPresentationProviderIdentityV1` is the immutable tuple
`(provider, nullable model, nullable effort, identityDigest)` derived from the
frozen occurrence binding with the common codec. Every Overview, Stages, Run
Inspector, occurrence-bound active-agent, and matched Timeline DTO carries that
same tuple together with `presentationRowId`, nullable `taskOccurrenceId`, and
nullable `occurrenceSequence`; a run-events Timeline row requires it null. Only
topology occurrence rows carry the authoritative
`planned|current|previous` position, and all other surfaces consume the exact
`OccurrencePresentationJoinV1` above. Semantic
selection keys by presentation row ID, never by agent ID. Timeline derives the
selected occurrence's `timelineLaneId`; its separate `Run events` selection
uses the run-events lane and no synthetic presentation row. Two same-agent
occurrences therefore remain distinct rows with independent status, attempts,
model, effort, and event lanes. Visual, Help, popover, copy, and accessibility
strings are generated from the same formatter result. Presence-aware decoders
reject a missing tuple, digest mismatch, or an execution/event tuple that
differs from the occurrence join; legacy attempts with null execution effort
still render the frozen effort byte-identically on all five surfaces.

Interactive targets are surface-qualified and never identify a non-row element
with a nullable row ID. `PresentationSubjectV1` is the required tagged union:

| Tag | Exact payload |
|---|---|
| `occurrence` | `presentation_row_id` |
| `timeline_event` | `timeline_lane_id`, `timeline_event_id` |
| `timeline_load_older` | `timeline_generation`, `snapshot_cursor_sha256`, `page_cursor_sha256` |
| `timeline_initial_loading` | `timeline_generation` |
| `timeline_initial_failure` | `timeline_generation`, `failure_code`, `request_digest` |
| `timeline_legacy_gap` | `source_digest`, `earliest_cursor_digest_or_empty` |
| `inspector_attempt_page` | `presentation_row_id`, `topology_snapshot_digest`, `attempt_page_generation`, `page_cursor_digest` |
| `stage_heading` | `stage_id` |
| `run_events` | `timeline_lane_id` |
| `loading_summary` | `load_generation` |
| `failed_summary` | `load_generation`, `failure_code` |
| `topology_unavailable_summary` | `load_generation`, `topology_failure_code` |
| `empty_summary` | no payload |

`RunWorkbenchDestinationV1` is the complete navigation enum
`overview | stages | artifacts | approvals | timeline | reports | system`.
`PresentationSurfaceV1` is separately closed to
`overview | stages | timeline | run_inspector | window_recovery`;
`PresentationSurfaceSlotV1` is closed to `primary | run_inspector`. The first
three surfaces occupy the
`primary` surface slot and Run Inspector occupies the independently mountable
`run_inspector` auxiliary slot. `window_recovery` is owned by the window and has
no surface slot or mount epoch. `artifacts`, `approvals`, `reports`, and
`system` are explicit
`unregistered(destination, requestEpoch, lastSurface?,
lastAcceptedMountEpoch?)` primary-slot
states for this proposal: they preserve semantic run selection and expose no
mounted-surface target inventory. The independently owned `window_recovery`
target is the sole proposal target/action permitted there. They may use their
existing local controls outside this protocol. Run Inspector
may remain mounted concurrently with any primary registered or unregistered
destination and has its own request/mount epochs and inventory.

`PresentationTargetV1` is the exact tuple
`(run_id, surface, subject, control_kind)`, where surface uses
`PresentationSurfaceV1`. Subject/control compatibility is the following closed
matrix; no generic `event` control exists:

| Subject | Legal control kinds |
|---|---|
| `occurrence` | `row`, `info`, `copy`, `popover`, `close` |
| `timeline_event` | `event_row`, `event_expand`, `event_collapse`, `event_copy_id`, `event_copy_raw` |
| `timeline_load_older` | `load_older`, `loading_older`, `retry_load_older` |
| `timeline_initial_loading` | `loading_timeline_initial` |
| `timeline_initial_failure` | `timeline_initial_failure`, `retry_timeline_initial` |
| `timeline_legacy_gap` | `legacy_gap_row` |
| `inspector_attempt_page` | `load_attempts_older`, `loading_attempts_older`, `retry_attempts_older` |
| `stage_heading` | `stage_heading` |
| `run_events` | `run_events` |
| `loading_summary` | `loading_summary` |
| `failed_summary` | `failed_summary`, `retry_load` |
| `topology_unavailable_summary` | `topology_unavailable_summary`, `restart_daemon_and_retry_topology`, `open_frozen_input_repair` |
| `empty_summary` | `empty_summary` |

The four `timeline_*` subjects are legal only on the mounted primary Timeline
surface. `timeline_initial_loading` requires the exact current
`loading_initial` or `gap_refetch` generation and is disabled but focusable.
`timeline_initial_failure` requires the exact current `failed_initial`
generation, failure code, and bounded request digest. Its
`retry_timeline_initial` control is legal only for `transport_unavailable` or
`timeline_snapshot_anchor_persistence_failed`; every other failure exposes only
the readable `timeline_initial_failure` control. `timeline_legacy_gap` requires
the current `ready.legacyGap` source digest and the SHA-256 digest of its
earliest reliable cursor bytes, or the fixed lowercase token `empty` when no
cursor exists; it is a readable row with no action. `timeline_load_older`'s
`load_older` requires current `ready` state, a non-null next-page
cursor, and exact generation/snapshot/page-cursor digests; `loading_older`
requires the exact in-flight request and is disabled; `retry_load_older`
requires `failed_retaining_rows` whose failed request is that same older page.
Activation captures the complete target before awaiting. Completion may mutate
only the matching Timeline generation and cursors; stale generation, snapshot,
page cursor, unmount, or destination change makes the callback a no-op. The
control remains a stable-height row above the oldest visible event so loading or
failure cannot shift event geometry.

`InspectorAttemptPageLoadStateV1` is the closed tagged union
`not_loaded | ready(presentationRowID, topologySnapshotDigest,
attemptPageGeneration, nextAttemptCursor?) |
loading_older(presentationRowID, topologySnapshotDigest,
attemptPageGeneration, pageCursorDigest, requestDigest) |
failed_retaining_attempts(presentationRowID, topologySnapshotDigest,
attemptPageGeneration, pageCursorDigest, failureCode, requestDigest)`.
`InspectorAttemptPageFailureCodeV1` is closed to `transport_unavailable`,
`response_schema_mismatch`, `topology_snapshot_expired`,
`attempt_page_cursor_invalid`, and `page_byte_limit_exceeded`; only
`transport_unavailable` is retryable in place. The page generation increments
whenever selection, run-load generation, or topology snapshot changes. The
`inspector_attempt_page` subject is legal only on the mounted Run Inspector
surface and only for the exact selected occurrence, snapshot digest, page
generation, and SHA-256 digest of the current opaque next-page cursor.
`load_attempts_older` requires `ready` with a non-null cursor;
`loading_attempts_older` requires the exact in-flight request and is disabled;
`retry_attempts_older` requires the exact retryable failed request. An
unselected row, stale page generation, changed snapshot, exhausted cursor, or
non-retryable failure has no attempt-page action. Completion can replace only
the matching Inspector attempt state and is a no-op after unmount, selection,
snapshot, generation, or cursor drift.

The `window_recovery` surface permits `failed_summary` for a run-load failure or
`topology_unavailable_summary` for a topology failure; those registrations are
mutually exclusive because failed run load requires topology `not_loaded`. A
failed summary permits exactly `retry_load`. A topology control is
`restart_daemon_and_retry_topology` exactly for a `restartable` disposition,
`open_frozen_input_repair` exactly for `replacement_required`, or
the noninteractive `topology_unavailable_summary` itself for `limit_only`;
every other pairing is
`presentation_target_invalid`.

`close` preserves the occurrence subject of the popover it closes. Every event
action therefore has a distinct target even when it acts on the same timeline
event. Its
machine identifier is exactly
`presentation_target_v1:<64 lowercase hex SHA-256 characters>`. The digest uses
the common length-prefixed UTF-8 codec with domain
`chainworks.presentation_target.v1` and exact ordered components
`[run_id, surface, subject_tag, canonical_base10(payload_count),
subject_payload_0 ... subject_payload_n, control_kind]`. Subject payload order is
frozen as the table above; integral generations use checked non-negative
canonical ASCII base-10. `TopologyFailureCodeV1` is closed to
`projection_absent`, `projection_schema_invalid`,
`projection_source_invalid`, `projection_order_invalid`,
`frozen_input_invalid`, and `projection_size_limit_exceeded`.
`TopologyRecoveryDispositionV1` is derived, never caller-supplied: the four
projection failures map to `restartable`, `frozen_input_invalid` maps only to
`replacement_required`, and `projection_size_limit_exceeded` maps only to
`limit_only`. Unknown surface,
subject, control, failure code, topology failure code, negative generation,
wrong payload count/order, or an incompatible
subject/control pair is `presentation_target_invalid` before hashing. Checked-in
known-answer vectors cover every legal subject and control, including both
event toggle directions and copy kinds, plus one mutation of each component.
Thus two
timeline events in one lane, two stage headings, or a heading and empty summary
can never compare equal. A row ID remains the cross-surface semantic selection
key, but it is never by itself a focus, popover, anchor,
accessibility-action, event, heading, or copy target.

`RunDetailLoadFailureCodeV1` is closed to `network`, `authorization`,
`daemon_schema_mismatch`, `response_schema_mismatch`, and
`storage_unavailable`. `RunDetailLoadStateV1` is the required tagged union
`idle | loading(runID, loadGeneration) | loaded(runID, loadGeneration) |
failed(runID, loadGeneration, failureCode)`. `RunDetailTopologyStateV1` is the
required tagged union
`not_loaded | available(runID, loadGeneration, projectionDigest) |
unavailable(runID, loadGeneration, topologyFailureCode)`. A loaded generation
has exactly one of the last two topology states with the same run/generation;
`idle`, `loading`, and `failed` require `not_loaded`. `RunSemanticSelectionV1` is
the required tagged union
`occurrence(presentationRowID) | run_events(timelineLaneID) |
loading(loadGeneration) | failed(loadGeneration, failureCode) |
topology_unavailable(loadGeneration, topologyFailureCode) | empty`.
`NavigationFocusIntentV1` is the closed tagged union
`primary(destination, requestEpoch, loadGeneration, activationSequence) |
auxiliary(run_inspector, requestEpoch, loadGeneration, activationSequence)`.
All integral values are checked non-negative per-window counters. A primary
destination in the auxiliary branch, `run_inspector` in the primary branch, or
any cross-branch slot/destination reconstruction is invalid. Only the two
explicit user-navigation events below may construct it; publication and
restoration code receives no constructor.
`SelectedRowSnapshotV1` stores the last selected occurrence row's
presentation-row ID, source stable ID, normalized index, occurrence position,
and stage ID. The single `RunOccurrenceSelectionStateV1` contains nullable
`selectedRunID`, non-null `selection: RunSemanticSelectionV1`, nullable
`selectedRowSnapshot`, non-null `loadState: RunDetailLoadStateV1`, non-null
`topologyState: RunDetailTopologyStateV1`, non-null
`timelineLoadState: TimelineLoadStateV1`, non-null
`inspectorAttemptPageState: InspectorAttemptPageLoadStateV1`, non-null
`activeDestination: RunWorkbenchDestinationV1`, fixed non-null
`surfaceSlots: SurfaceSlotsV1`, nullable
`popoverTarget: PresentationTargetV1`, and nullable
`pendingNavigationFocusIntent: NavigationFocusIntentV1`, nullable
`loadRecoveryRegistration: RunLoadRecoveryRegistrationV1`, nullable
`topologyRecoveryRegistration: WindowTopologyRecoveryRegistrationV1`, nullable
`focusOwner: FocusOwnerV1` plus nullable
`focusRegistration: FocusRegistrationV1`. `FocusOwnerV1` is the closed union
`surface(slot) | window_load_recovery | window_topology_recovery`;
`FocusRegistrationV1` is the closed union
`mounted(MountedTargetRegistrationV1) |
window_load_recovery(RunLoadRecoveryRegistrationV1) |
window_topology_recovery(WindowTopologyRecoveryRegistrationV1)`. Owner and
registration are both null or are the matching branch; a surface owner must
equal the mounted registration's slot. Exactly one load/topology recovery
registration exists when its matching state requires it, and they can never
coexist. The snapshot is non-null only for
`occurrence`; all other selections clear it. `loading` and `failed` are legal
only when their generation and failure code exactly match `loadState`;
`topology_unavailable` is legal only when its generation/code exactly match
`topologyState`; `empty` is legal only for a matching `loaded` generation with
`topologyState = available` and no selectable occurrence row or run-events
lane. A reducer may retain a surviving stage heading as a semantic fallback
candidate, but that candidate is not focus and cannot construct a focus intent.
`empty` is
forbidden while topology is unavailable. There is no per-run dictionary,
nullable row-ID surrogate for a non-row selection, or view-local
`selectedAgentID`.

`SurfaceSlotStateV1` is
`unmounted(requestEpoch, lastSurface?, lastAcceptedMountEpoch?) |
requested(surface, requestEpoch, lastAcceptedMountEpoch?) |
registered(MountedSurfaceInventoryV1) |
unregistered(destination, requestEpoch, lastSurface?,
lastAcceptedMountEpoch?)`. `SurfaceSlotsV1` has exactly two
fields, `primary` and `runInspector`, each with an independent non-negative
request epoch. `primary` accepts registered surfaces only for Overview, Stages,
and Timeline or an explicit unregistered state for the other four destinations;
`runInspector` accepts only unmounted/requested/registered Run Inspector. Thus a
primary Timeline inventory and an Inspector inventory can coexist and neither
overwrites the other's targets or epoch. A new request epoch or run-load
generation sets `lastAcceptedMountEpoch = null`; unmount within the same tuple
preserves the last accepted surface and epoch as a tombstone. The next mount for
that tuple must therefore use the exact successor epoch and can never reuse
epoch 0.

`WindowTopologyRecoveryRegistrationV1` is owned by the per-window publication
owner rather than a mounted surface. It contains one complete recovery target,
run/load generation, topology failure code, derived recovery disposition, and
window routing ID. It exists iff the current topology state is unavailable and
is rendered by the window-level recovery presenter above every one of the seven
primary destinations and Run Inspector. It remains keyboard-focusable even
when the active destination has no proposal-owned mounted inventory, so an
unregistered tab cannot hide the only recovery path.

`RunLoadRecoveryRegistrationV1` is likewise window-owned and exists iff
`RunDetailLoadStateV1` is `failed`. It stores the exact failed run ID,
load-generation, failure code, original bounded load request digest, window
routing ID, and complete `window_recovery/failed_summary/retry_load` target. The
window-level presenter renders it above every one of the seven primary
destinations and Run Inspector, including all four unregistered destinations.
Activation rechecks the complete tuple, allocates a strictly greater
load-generation, clears only stale run-detail/topology/Timeline publication for
that old generation, and reissues the same bounded load through the production
client. Repeated activation while the new generation is loading is idempotent;
an old callback or target is ignored. Hosted tests fail the initial load from
each destination with Inspector both open and closed, keyboard-focus and invoke
the window target, and prove one new generation and no hidden or duplicate
retry.

`MountedTargetRegistrationV1` contains the complete target, `loadGeneration`,
`surfaceSlot`, `surfaceRequestEpoch`, non-negative `surfaceMountEpoch`,
`surfaceMountToken`, and `mountToken`. `MountedSurfaceInventoryV1` contains one
run/slot/surface/load/request/mount tuple, its `surfaceMountToken`, and the
duplicate-free map from target to registration. The surface token is exactly
`presentation_surface_mount_v1:<sha256>` over common-codec components
`[run_id, surface_slot, surface, canonical_base10(load_generation),
canonical_base10(surface_request_epoch), canonical_base10(surface_mount_epoch)]`
with domain `chainworks.presentation_surface_mount.v1`. A target token is exactly
`presentation_mount_v1:<sha256>` over common-codec components
`[surface_mount_token, presentation_target_identifier]` with domain
`chainworks.presentation_mount.v1`. Negative numbers, a target for another
run/surface/slot, a duplicate target, either token mismatch, or an inventory from
an older request/load generation is rejected as `presentation_mount_invalid`.

`P031RunsHomeViewModel` owns one `RunsWorkbenchPresentationModel`. Overview,
Stages, Timeline, Run Inspector, popover, and focus events all call one pure
`RunOccurrenceSelectionReducer.reduce(state:event:rows:)`; each mounted user
event carries its complete `PresentationTargetV1`, target mount token, and
surface mount token, and views receive only reducer bindings/actions. The four
unregistered destinations emit only navigation/load events and cannot fabricate
mounted-surface proposal targets; they may only render/activate the current
window-owned load or topology recovery registration. A successful load with available topology chooses the new
run's first `current`, first `previous`, then first planned occurrence; if none
exists it chooses the run-events lane, then `empty`. A successful load with
unavailable topology instead selects the exact generation-qualified
`topology_unavailable` state and exposes no guessed rows or `empty` state.

Construction is exact and per window. Each scene creates one immutable
`RunWindowRoutingIDV1`, spelled `run_window_v1:<32 lowercase hex>` from 128
cryptographically random bits, and registers exactly one `RunWindowCommandRouter`
for that ID until scene teardown. SwiftUI app commands resolve the router only
through the key window's `@FocusedValue`; a command with no focused router is
disabled. A run deep link targets the key/focused Chainworks window when one
exists, otherwise opens exactly one new run window and binds its pending route
to the new window routing ID before publication. Proposal-owned commands,
direct-open requests, and deep links never use a process-wide
`NotificationCenter` broadcast or enumerate all view models. Two-window hosted
fixtures keep different runs and destinations open, issue each command/deep
link, and prove exactly one routing ID mutates while the background window's
load generation, selection, focus, and subscriptions remain byte-identical. A
source gate rejects proposal-owned `NotificationCenter` command/deep-link
observers and any router call without an explicit routing ID or focused scene.

`ContentView` creates one
`@StateObject P031RunsHomeViewModel` from the application dependency container
and injects that instance into `RunsHomeView`, every run-detail destination,
the Run Inspector scene, direct-run presentation, and deep-link handlers.
`RunsHomeView` changes to an initializer requiring the injected model and has no
production default constructor or second `@StateObject`. Direct surfaces and
resolved per-window deep links call
`openRun(runID:destination:routingID:)` on that same object instead of
constructing a workbench model; a mismatched routing ID is rejected. The view
model exclusively constructs its one
`RunsWorkbenchPresentationModel` and one `RunDetailPublicationOwner`; load-
generation counters, navigation-activation counters, observation tasks,
snapshot pagination, subscriptions, and selection state all live there and are
cancelled on window teardown. Run Inspector receives the same reference, not a
copied read model. A recursive production call-site gate permits those three
constructors only at this ownership chain, while previews/tests use an explicit
fixture factory. Hosted tests exercise ordinary Runs navigation, direct open,
deep link, counters, observers, and concurrent Inspector and prove one initial
load, one timeline snapshot/subscription pair, and one selection/focus reducer
lifecycle per window.

For occurrence selection, `rowsChanged` uses the retained snapshot, not a lookup
in the already-replaced row array: it first maps a removed planned row to the
current row with the same source stable ID; otherwise it chooses the row now
occupying the prior normalized index, then the last preceding row, then the
run-events lane when present, then `empty`. It may retain that stage's surviving
heading only as a semantic fallback candidate for a later explicit user
navigation activation; `rowsChanged` itself never focuses it or constructs a
navigation focus intent. For `run_events`, occurrence row churn does
not change selection while the exact lane exists; disappearance chooses the same
occurrence default and then `empty`. For `empty`, newly available data chooses
the same occurrence default and then run-events. These three reductions are
legal only while the matching topology state is `available`. `loading`,
`failed`, and `topology_unavailable` ignore row churn and may transition only
from the matching publication-owner generation. A timeline event action under
an occurrence lane selects that occurrence; one under the run-events lane
selects `run_events`, while the event-specific target remains the independent
focus/action target. Popover and focus transitions do not replace semantic
selection. The reducer resolves focus only within the current owner slot and
closes a popover whose exact target no longer exists in either registered
inventory; it never transfers a `stages` anchor to `overview` merely because the
row ID matches. If the focused row or control disappears during background row
replacement, the source-qualified focus registration is cleared and focus
becomes nil; no heading, summary, other row, or other slot receives focus until
an explicit user navigation activation. Every selected occurrence and successful occurrence fallback
refreshes the row snapshot; every non-occurrence selection clears it.

Primary navigation is deliberately two-phase. A user activation emits
`primaryDestinationRequested(destination, navigationActivationSequence)`: the
reducer increments only the primary request epoch, sets `activeDestination`,
stores one `NavigationFocusIntentV1.primary(destination, requestEpoch,
loadGeneration, navigationActivationSequence)`, clears only the prior primary
inventory, resets that new request's accepted-mount tombstone to null, and
preserves semantic selection and any independently mounted Run Inspector
inventory. The activation sequence is a checked monotonic per-window counter;
background publication, subscription delivery, restoration, and row changes
cannot create an intent. For Overview, Stages, or Timeline the reducer records
`requested(surface, epoch, null)` and waits for that view to mount. For Artifacts,
Approvals, Reports, or System it immediately records
`unregistered(destination, epoch, priorSurface, null)` and consumes the intent through
the current window load-recovery registration when run load failed or topology-
recovery registration when topology is unavailable;
otherwise `ExistingDestinationFocusBridgeV1` focuses that destination's existing
root control by the closed key `artifactsRoot | approvalsRoot | reportsRoot |
systemRoot`. If primary previously owned mounted proposal focus it clears that
registration first. This supplies deterministic focus entry for all seven
destinations without fabricating proposal targets for the four unregistered
surfaces. Opening Run Inspector uses
`auxiliarySurfaceRequested(run_inspector, navigationActivationSequence)` and
stores only `NavigationFocusIntentV1.auxiliary(run_inspector, requestEpoch,
loadGeneration, navigationActivationSequence)` for the auxiliary slot; closing uses
`auxiliarySurfaceClosed` and changes only that slot.

Only a requested registered slot or the same tuple's tombstoned `unmounted`
slot may emit
`surfaceMounted(slot, surface, requestEpoch, loadGeneration, mountEpoch,
surfaceMountToken, registrations)`, where registrations are the exact MainActor
inventory for that mount. For a new request/load tuple the first accepted mount
epoch is exactly `0`. For a tuple whose inventory was unmounted, the preserved
tombstone makes the next accepted mount epoch exactly
`lastAcceptedMountEpoch + 1`; epoch 0 reuse is rejected. A repeated event at the current epoch is an idempotent
no-op only when the canonical inventory bytes, surface token, and every target
token are byte-identical. A changed target set must use exactly current epoch
plus one. Lower epochs, gaps, same-epoch mutations, overflow, wrong slot/surface,
or wrong request/load tuple are rejected as `presentation_mount_invalid` and do
not partially replace the inventory. Each accepted successor republishes the
complete inventory; it never patches registrations or reuses prior-epoch target
tokens. Mounting cannot request another surface, so there is no request/mount
feedback cycle.

Accepting or republishing a mount never by itself changes focus ownership. If
the currently focused registration survives byte-identically, it is preserved,
including when the other slot publishes. If it disappears without a user
activation, focus is cleared rather than transferred to another slot. Only a
pending navigation focus intent whose branch, destination/surface, request
epoch, load generation, and activation sequence all match may be consumed. If
run load is failed, the user-authorized reduction first chooses the exact
window-owned load-recovery registration. Otherwise, if topology is unavailable,
it chooses the exact window-owned topology-recovery registration, regardless of
whether the destination has a mounted surface. Otherwise, on mount it chooses a registration in this exact
order: the same valid occurrence control, the selected run-events lane, the
matching generation-qualified loading/failed summary, the loaded empty summary,
then the selected occurrence's stage heading or, for `empty`, the remembered
stage-heading fallback candidate. That final heading path is reachable only
while consuming this explicit user-navigation intent; background `rowsChanged`
has no constructor for it. If none exists, focus is nil but semantic selection
remains unchanged and the intent is consumed. It never
manufactures a target from row identity alone. Every reducer action originating in a mounted control carries its exact
registration and source surface token; it is accepted only if the latest
inventory in that slot contains both byte-identically.

`RunFocusBridgeV1` is the sole bidirectional bridge between reducer focus and
SwiftUI `@FocusState`. On the MainActor it applies a reducer-requested
registration only when both its target and surface tokens remain current and it
equals the latest desired focus. An actual non-null focus gain emits
`focusChanged(registration, sourceSurfaceMountToken)`. An actual focus loss emits
`focusLost(previousRegistrationToken, sourceSurfaceMountToken)`; it never emits
an unqualified `nil`. The reducer accepts gain only from the inventory carrying
that exact registration and accepts loss only when both values identify the
currently focused registration and its current source surface. The
`window_recovery` focus branches are driven only by the current matching
`RunLoadRecoveryRegistrationV1` or
`WindowTopologyRecoveryRegistrationV1`; neither is converted into a mounted
surface registration, and a stale recovery callback cannot clear surface focus.
A stale loss
from a replaced/unmounted surface therefore cannot clear focus gained on the
other concurrent slot. A monotonically increasing focus-application token
suppresses the matching programmatic callback without suppressing later user
focus changes. Unmount emits
`surfaceUnmounted(slot, sourceSurfaceMountToken)` and transitions only the exact
matching inventory to `unmounted`, preserving its surface and accepted mount
epoch tombstone; it clears focus only when that slot/token owns it. Surface
request, generation change, row replacement, and auxiliary close reconcile
through the same bridge. Pure reducer and hosted tests cover both directions,
feedback-loop suppression, new-tuple epoch 0, post-unmount exact successor,
same-epoch replay/gap/lower epoch, request-before-mount,
mount-before-stale-unmount, source-qualified stale
focus loss, concurrent primary/Inspector focus changes, a disappearing focused
row, all seven destinations, and loading, failed, topology-unavailable, empty,
occurrence-selected, and run-events-selected states.

The view model also owns one `RunDetailPublicationOwner`. Beginning a run load
atomically increments monotonic `load_generation` and `timeline_generation`,
cancels the prior request and subscriptions, clears prior run-detail rows, sets
`loadState = loading(runID, loadGeneration)`, sets semantic selection to the
same generation-qualified `loading`, sets topology state `not_loaded`, sets
Timeline state `loading_initial(timelineGeneration)`, dismisses
stale popover state, clears both prior window recovery registrations and any
topology-restart operation,
and invalidates both slots' load-bound inventories/focus
before starting the new request. Registered slots preserve their independent
request epochs but must remount at epoch 0 for the new load generation;
unregistered primary state remains explicit and emits no mount. It never exposes
`empty` between runs. Every initial GraphQL response, subscription update,
raw-detail response, and topology retry callback carries its captured
`(run_id, load_generation)`. Publication is accepted only when both values
equal the owner's current tuple; a stale callback is dropped without mutating
rows, selected snapshot, popover, focus, timeline lane, or error state. A
successful replacement subscription is installed only under the same CAS and
old-generation cancellation cannot clear the new subscription. A successful
initial response validates topology before rows. A valid projection atomically
sets matching `loaded` plus `topologyState = available`, publishes rows, and
applies the deterministic occurrence/run-events/empty rule with no recovery
registration. A missing or invalid
projection atomically sets matching `loaded` plus
`topologyState = unavailable`, publishes no topology rows, and selects the exact
generation/code-qualified topology-unavailable state while publishing the exact
topology recovery registration and derived disposition. It can never reduce that
response to `empty`. A current-generation subscription that detects topology
corruption performs the same one-way available-to-unavailable reduction, clears
rows/snapshot/popover, invalidates their registrations, and publishes the
matching topology recovery registration. Once unavailable,
that generation cannot return to available; only the restart/new-generation path
below can recover. A terminal load error sets matching `failed` and publishes
the exact `RunLoadRecoveryRegistrationV1`; no mounted failed-summary target is
required. Its `retry_load` control is accepted only with the current window
routing ID, target, request digest, and exact failure tuple; it always begins
one new generation and cannot mutate/reuse the failed generation. Pure and
hosted fixtures delay run A, select run B, publish B, then
deliver A responses, errors, focus callbacks, and updates in every order; B
remains byte-identical and no A target can reappear.

The same publication CAS owns Timeline history without coupling it to run or
topology publication. A valid initial run response may publish available or
topology-unavailable state immediately while Timeline independently enters
`loading_initial` and registers exactly one
`timeline_initial_loading/loading_timeline_initial` target. A current initial
failure atomically replaces it with the exact
`timeline_initial_failure/timeline_initial_failure` target and, only for either
retryable code, the `retry_timeline_initial` target. The first authorized
durable Timeline page atomically removes those targets and is then published,
its durable cursor is subscribed before any older-page request, and replay/live
rows are deduplicated into the lane inventory. Older pages load only after an
explicit exact `timeline_load_older/load_older` target. The in-flight target is
replaced by `loading_older`; a retained-row page failure exposes only
`retry_load_older` for the same cursor tuple. Every page and subscription event carries
`(run_id, load_generation, timeline_generation, snapshot_cursor)`; another tuple
is stale. `requiresFullRefetch`, cursor gap, page digest mismatch, or
subscription overflow increments `timeline_generation`, retains the current run
load, topology, semantic selection, and focus state, and replaces only Timeline
rows after the new first-page/subscription handoff validates. It never appends
across the gap. The owner enforces the 512-event/8-MiB aggregate and
256-event/1-MiB live-buffer caps above, evicting only resumable non-visible
historical pages. A ready legacy gap publishes exactly one
`timeline_legacy_gap/legacy_gap_row` target in the Timeline inventory. Focus
order is initial status first while no rows exist; otherwise lane/event controls
in durable display order, then the legacy-gap row when present, then the
older-page control. A run with no live activity still publishes its first durable
snapshot page, and a zero-event snapshot follows the exact lane rules above.

The topology-unavailable action is accepted only from the current
`WindowTopologyRecoveryRegistrationV1` and matching window router. For a
`restartable` disposition, acceptance atomically creates one
`TopologyRestartRequestV1` containing random operation ID, window routing ID,
run ID, topology failure code, load generation, recovery-target identifier, and
current daemon epoch. The exact tuple is stored in the publication owner before
invoking the production daemon lifecycle restart coordinator; same-target replay
returns the same operation, while a different target/generation/router is
stale. It remains on the unavailable state while restart is pending or failed.
A callback may begin a new load generation only when operation ID and every
captured run/load/target/router value still match current state and the returned
healthy daemon epoch is strictly greater than the captured epoch. Window
teardown, run switch, or another load clears the pending operation and makes its
callback a byte-for-byte no-op; ordinary tab navigation does not hide or cancel
the window-owned topology recovery registration. The recovery response must carry that
new generation and a valid projection before the reducer may choose an
occurrence, run-events lane, or `empty`; callbacks from the unavailable
generation are ignored.

For `replacement_required`, the current window router accepts only
`open_frozen_input_repair` and constructs one `FrozenInputRepairRouteV1` with
the exact tuple `(operationID, windowRoutingID, runID, loadGeneration,
frozenWorkflowSnapshotID, frozenWorkflowSnapshotDigest, topologyFailureCode,
recoveryTargetID)`. `operationID` is allocated once per accepted recovery
target; all other fields are copied from the current window registration and
frozen run readback, never reconstructed from a selected row. The scene-scoped
`AppShellNavigationCoordinatorV1` accepts this typed route only for the same
window routing ID, opens the Definitions destination at the immutable frozen
workflow revision, and starts the existing replacement-run flow in that same
window with the route tuple as its prevalidated input. It never edits the
original run snapshot. The coordinator journals the accepted operation before
navigation; replay of the same complete tuple returns the same operation and
does not open a second window, destination, or replacement flow, while any
field mismatch is stale. It records no daemon-restart operation. Two-window
hosted fixtures prove the background window remains byte-identical and that a
direct Run Inspector activation follows the same typed route. Run Inspector and every primary
destination consume the same window registration and never retain an occurrence
summary while topology is unavailable. Fixtures deliver restart
success/failure callbacks across every stale router/target/operation/load
permutation and prove frozen-input activation cannot enter a restart loop.

For `limit_only`, the summary has no activation. It records no restart,
replacement, report, or load operation; keyboard focus may read the exact
guidance but cannot manufacture a command.

Run Inspector deletes `activeTimelineAgents.first`. For `occurrence` selection
it resolves that exact `RunStageTopologyOccurrenceV2`, shows its currently
retained paged attempt window ordered by
`started_at DESC, agent_execution_id DESC`, exposes only the exact legal
`inspector_attempt_page` control for its current
`InspectorAttemptPageLoadStateV1`, and uses its
latest attempt only for the summary identity. For `run_events` it shows the
selected run-events lane and no occurrence summary or attempt fallback; for
`loading`, `failed`, `topology_unavailable`, or `empty` it shows only the
matching generation-qualified loading/failed/topology-unavailable or
loaded-empty summary. Initial/default occurrence choice
belongs only to the reducer rule above. Popover and focus consume that exact reducer
result rather than implementing a second fallback rule. Inspector focus order
is occurrence summary, retained attempts in display order, then the legal
attempt-page control. An agent ID is never a
selection key. Pure reducer tests cover all surface/control pairs,
same-row targets on two surfaces, removed-row snapshot fallback, planned to
current replacement, popover invalidation, run switching, run-events lane,
generation-qualified loading/error, and every request/mount/unmount state.
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
timeline expand/collapse, copy-ID/copy-raw focus, bidirectional user focus,
all seven destinations, concurrent primary/Inspector focus, exact mount-epoch
validation, source-qualified focus loss, loading-to-loaded,
topology-unavailable-to-new-generation recovery, failed-to-retry, and stale run-A
publication after run B each fail if the modifier is absent or attached to
another target.

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
| Cancellation wins before configuration acceptance | `cancelled_before_configuration` | 0 |
| Cancellation/deadline/transport loss wins after acceptance but before prompt dispatch | matching typed `ProviderPostConfigurationOutcomeCodeV1`; accepted receipt retained | 0 |
| Original-owner reused generation evidence mismatch | close generation and negotiate fresh once | 0 on old session |
| P079 repair/fallback or P086 generation evidence mismatch | fail typed owner; transparent fresh fallback forbidden | 0 |
| P079 fallback provenance/operation/lease join mismatch | `ACP_PROMPT_OWNER_INVALID`; settle fallback attempt | 0 |
| P079 candidate staging write/digest/fsync fails | no prepared set; parent/transition remain held; typed validation-settlement failure | unchanged |
| P079 artifact set/member is `prepared`, `history_committed`, or `destination_committed` after crash | startup reconciler restores the current activation from immutable history, publishes/quarantines every member, and completes the same set; no validation/prompt replay | unchanged |
| P079 canonical activation CAS loses to another completed repair | preserve immutable history, settle `canonical_activation_conflict`, keep parent/transition blocked, and restore bytes from the winning activation | unchanged |
| P086 continuation lacks execution/occurrence/turn/work-item binding | `ACP_PROMPT_OWNER_INVALID` | 0 |
| P086 resume context missing/digest or target binding mismatch | admission rejected before launch | 0 |
| P086 setup deadline expires before broker/spawn | settle reserved process intent as never launched; reconcile the configuration journal and write `configuration_deadline_elapsed` only when no acceptance commit exists | 0 |
| P086 setup deadline expires after launch but before `prompt_sent` | stop setup work; identity-safe reap; reconcile the exact configuration journal, replay a committed failure, or preserve a committed receipt plus append `configured_deadline_before_prompt`; unresolved journal is failed-serve | 0 |
| P086 execution watchdog expires after `prompt_sent` | preserve sent truth and use the ordinary 300/900-second watchdog terminal/cleanup reducer; the 30-second setup window is not consulted | already sent |
| P086 startup observes another boot ID or a legacy active row without V1 monotonic clock | cleanup/zero-prompt settlement only; `legacy_resurrection_window_unverifiable` for legacy; no provider work or replacement window | 0 |
| P086 cleanup deadline expires with unresolved process/authority | close first fatal; startup runs only `P086ExpiredWindowReconcilerV1` against the same immutable window, never provider work or deadline replacement | 0 |
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
| Unsupported/malformed runtime receipt | `runtime_receipt_invalid`; no projection | preserve turn ledger |
| Topology execution lacks unambiguous occurrence identity | omit execution association; expose legacy ambiguity | unchanged |
| Schema v1 probe or selected-key contract fails | typed daemon schema mismatch; no reduced query | unchanged |
| Legacy generic frozen run | allowed as planned/unverified | shared ledger for each new attempt |
| Runtime lock is `duplicate_healthy`, `anomalous_holder`, or `lock_failure` | typed outer bootstrap exit; no router, listener, DB open, or mutation | 0 |
| Migration 100 incomplete while migration 101 is already applied | `provider_truth_future_migration_applied_before_phase_complete`; operator-only corruption, no automatic repair/rollback/adoption | 0 |

Configuration failures use
`ProviderFailurePresentationV1.phase = provider_configuration`, leave accepted
fields `null`, create no `AcpRuntimeReceipt`, and may render the requested pair
plus `No prompt sent`. Post-configuration zero-prompt outcomes preserve the
accepted receipt and use their closed outcome code without creating a runtime
receipt. Dispatch failures use
`ProviderFailurePresentationV1.phase = prompt_dispatch`,
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
| `workflow` + `domain` | Exact seven-profile matrix; per-profile frozen capabilities and effective fallback contract; validated Steward catalog; fresh generic/invalid rejection; legacy replay; sealed provenance union including typed P079 fallback; exact ten-ID manifest with occurrence-free P017 identity branch and occurrence-bound other producers; typed P058 owner; `OutputContractRepair` work-item kind; complete compiled-coordinate, condition, binding, dynamic-key, deterministic enqueue-time occurrence allocation, unique human-source ordinal, legacy ordering, sequence, and presentation vectors |
| `acp` fake provider + `engine` dispatch | Response-closed negotiation plus generation-owned `config_option_update` snapshot; class-tagged receipt/readiness permits and complete provider-class x owner-kind dispatch matrix; one private `GenerationReservationWriterV1` invoked only inside the five registered enclosing operation families, including contained P079 repair/fallback admission and output-only conversion; enforced Seatbelt staging containment with no external MCP, direct-write, syscall-mode, and descendant escape negatives; P086 admission-time opaque session ref, immutable descriptor/inode/mount-bound root authorities enforced by Seatbelt, and context/window with boot-session plus continuous-time 30-second setup and 10-second pre-prompt cleanup authority, prompt deadline `min(setup, write+10s)`, ordinary 300/900-second post-send watchdog, sleep/wall-jump/reboot/legacy-clock reduction, bounded cursor-addressed post-expiry cleanup-only restart, configuration-journal-first cleanup, typed post-readiness outcome preservation, replacement-turn output-only conversion, post-launch capability proof, exact Claude `session/new.params.resumeSessionId` attach and zero-send unsupported Codex branch, pre-response-update rejection, response catalog, ordered post-response updates, and parent-aware same-process/restart process cleanup with PID-reuse proof; verified immutable private launch closure and trusted suspended launch gate remain inert until durable binding, resolve non-serializable secrets only under the release permit, and bind proc identity before initialize; raw-session sentinel absent from new authority/log/error evidence while existing authorized raw-ID projections and fixed-salt `SessionGeneration.providerSessionRef` remain byte-compatible and never contain `psref_*`; separate new/existing-generation reservations; many-to-one ownership with one prompt-through-terminal manager; acyclic typed authority/control ports and permit-only API; bounded broker/config/send/terminal-settlement/cleanup; complete stage-less P017/P058/P079/P086/Steward reducers and collateral matrix; executor loads the Steward claim-created turn and only the sealed turn-0 reservation permit may create its generation; owner-scoped versus generation cancellation; frozen deterministic fallback selection; Claude aliases unchanged |
| `db` + `engine` recovery | Stable-inode outer lock acquired before any principal-table mutation plus guarded phased migration that skips the filtered source after phase completion, refuses an early 101 ledger as operator corruption, and accepts a full 101+ ledger on a second clean restart; complete registered Class A operation/result-codec/replay-rule set including separate receipt/readiness and P079 no-candidate settlement, immutable parent/member/absence witnesses, closed successor graphs, 0/1/512-member replay, fixed 512-permit reconciliation registry with terminal/fatal reserves, P086 cleanup-deadline-bounded late-commit handoff, global result sequence/hash chain, durable pending envelopes, 256-row/4-MiB/15-second startup batches, restart high-water continuation, and one-million-row no-rescan proof; barrier-before-SQLite global writer order and append-only fatal-cycle reconciliation before reopen; separate acknowledgement certainty and operation-specific domain outcomes; exact generation-binding receipt/readiness/failure/cancellation/post-outcome matrix plus append-only cleanup events; active owner attempts/receipts/invalidations/failures/cancellations; sealed Steward analysis/activation/retry generation constructors and cancellation reducer; prompt authority/quarantine; immutable migration-095 checksum plus complete migration-100 P079 DDL and exact `P079SchemaManifestV1`, validation/no-candidate evidence, settlement allocator/checkpoint, native-only logical and canonical-path uniqueness, deterministic one-winner migrated authority, independent lease/fallback quarantine accounting, immutable artifact history, per-member history/destination Class A operations, final DB-only completion, canonical activation CAS, and delete/update guards; immutable P058 execution/ledger/tier authority; exhaustive provider-session correlation manifest and purpose-limited private raw-ID resolver with no new public opaque ref; deterministic occurrence enqueue/copy-validation, unique human stage/task/event ordinals, durable Timeline event journal/event-or-empty-anchor cursor, honest legacy-gap evidence, bounded backfill/checkpoint, and restart; exhaustive old P086 classifier; sealed legacy envelopes; closure-owned replay authorization |
| `daemon` composition | Closed outer `acquired|duplicate_healthy|anomalous_holder|lock_failure` result over one persistent mode-0600 lock inode; three-process contention and source scan prove no unlink/replace and only one guard; only acquired binds one starting router and enters guarded `ready|preflight_failed` bootstrap; failed owner retains `PreflightLockGuard`; production construction of upgrade coordinator, durable authority, ACP manager, invocation/invalidation coordinators, process-control port, `DbWriter`, artifact-set reconciler, and sole `FirstFatalCoordinator`; global barrier-before-SQLite order, immutable first-fatal cycle persist-before-notify, restart reconciliation, and no deadlock; exact Operator-only shipped `P031DaemonStatus { daemonStatus { json } }` AST/body whitelist and zero-DB minimal routes; production `DaemonLifecycleClient` uses the named `P031URLSessionGraphQLReadTransport` request for starting/failed polling and live-principal revocation tests |
| `graphql-server` | Byte-equal complete `AppSchema::sdl()` with explicit lowercase snake-case enum literals, uppercase/unknown negatives, probe matrix, and exact schema-version literals; one non-null execution-level truth object plus turn-owned configuration truth; complete latest-specialized-turn reducer; simultaneous P079 source-generation-A/contained-generation-B and P086 target-generation-A/attached-generation-B readback; no new `providerSessionRefId` or public `psref_*` field, while retained P046 `SessionGeneration.providerSessionRef` keeps its exact nullability, authorization, fixed-salt, and restart-instability vectors; bounded paged topology and occurrence-attempt documents plus typed legacy-over-limit rejection; strict Timeline run-ID parsing and complete typed HTTP/WebSocket error matrix; authorized paged durable Timeline snapshot plus event/empty-anchor cursor handoff, honest legacy gap, replay/refetch, and old `rte_` compatibility vectors; one zero-event lane per occurrence and a run-events lane only when unassociated events exist; all six raw-detail status/nullability rows including null-identity missing/unauthorized and authorized unassociated-run-event branches; mediation/topology mapping; structural proof that this slice does not change MCP/report/resource schemas |
| Swift focused and hosted-view tests | Complete production `P031GraphQLDocumentSet.providerExecutionTruthSchemaProbe`/`runDetail`/`runStageTopologyPage`/`occurrenceExecutionAttemptPage`/`runtimeTimelineSnapshot`/`runtimeStatusChanged`/`timelineRawDetail`/`daemonStatus` snapshots for named operations and presence-aware DTO/error decoding; bounded paged topology/attempt accumulation, typed over-limit/limit-only recovery, field-level topology recovery, and strict non-topology decode; conditional raw-detail join after six-row nullability validation; first-page Timeline publication, immediate event-or-empty-anchor cursor handoff, honest legacy-gap row, explicit typed load-older/loading/retry controls, dedupe, Timeline-only gap refetch, and client caps; lockstep restart; complete planned-shell and typed state/ambiguity/start-failure/invalidation/post-outcome matrices with byte-distinct pre/post-acceptance cancellation; exact bounded diagnostic visual/Help/copy strings and separately safe accessibility identity from one formatter plus independent stdlib oracle; one random routing ID, focused-scene command router, and publication owner per window across ordinary/direct/deep-link flow; generation-qualified loading/loaded/failed/topology-unavailable publication and semantic selection; all seven navigation destinations, explicit unregistered primary tabs with deterministic existing-root focus entry, tagged primary/auxiliary focus intent, concurrent primary/Run Inspector slots, user-activation-only focus transfer, new-tuple epoch-0/tombstoned-successor/idempotent-replay mount rules, source-qualified focus gain/loss, and bidirectional MainActor focus bridge; exact `presentation_target_v1`, `presentation_surface_mount_v1`, and `presentation_mount_v1` known-answer codecs; distinct event row/expand/collapse/copy-ID/copy-raw targets, bounded human labels, visible/spoken legacy-order state, human stage/task/event ordinals, and clipboard bytes; separate window-owned all-destination run-load and topology restart/replacement/limit-only states; exhaustive subject/control accessibility matrix and exact 16-case 920x620/1440x900 layout matrix with occurrence/event discriminators only where legal; opaque generated identities only in machine identifiers and never spoken; exact topology rules; real `NSHostingView`/`.focused` first-responder proof, without claiming remote keyboard/VoiceOver event delivery |

Swift proof is two independent invocations and result bundles:

1. `codex-model-truth-pure.xcresult` runs
   `ProviderExecutionIdentityFormatterTests`,
   `ProviderExecutionTruthDecodingTests`,
   `ProviderPromptConfigurationTruthDecodingTests`,
   `P031RunDetailContractTests`,
   `P031TopologyPaginationContractTests`,
   `P031TimelineIdentityContractTests`,
   `P031TimelineGraphQLErrorContractTests`,
   `RunOccurrenceSelectionReducerTests`,
   `RunRecoveryRegistrationTests`, and
   `StageTopologyLayoutBuilderV2Tests`.
2. `codex-model-truth-hosted.xcresult` runs
   `RunModelIdentityHostedTests`, `RunStageTopologyHostedTests`, and
   `RunResponsiveLayoutHostedTests` against the real run-detail views.

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
5. accepted-truth receipt persistence, provider-neutral readiness,
   invalidation persistence, and separate failure/pre-acceptance-cancellation/
   post-configuration settlement faults prove the exact class-specific evidence
   matrix, no invented runtime receipt, the exact prompt count, and synchronous
   fatal admission-fence behavior; the generated provider-class x owner-kind
   fixture covers ordinary/P058/fallback, P017, P079 repair, P086, and both
   Steward lanes, consumes one monotonic attempt for every cell, and rejects a
   missing/wrong receipt, readiness, failure, cancellation, or current pointer;
6. a legacy generic request proves the old best-effort path remains reachable,
   while Claude alias matching remains byte-compatible;
7. `reserve_new_generation` and `reserve_existing_generation` races prove one
    globally unique prompt-turn binding, idempotent same-generation replay,
    conflicting cross-generation reuse, monotonic owner configuration-attempt
    allocation with no replay/loser gaps, zero loser I/O, and every legal/illegal
    `admitted` through terminal class-specific receipt/readiness-versus-failure-
    versus-cancellation reference tuple;
8. matching existing-generation evidence first creates an admitted binding with
    no success evidence, then exactly one registered settlement derives either
    an exact owner receipt or a non-exact owner-bound readiness reference without
    changing the physical generation/readiness cardinality; both consume exactly
    one owner configuration attempt and produce only their matching class-tagged
    dispatch permit. Missing/mismatched evidence closes the old handle and
    permits only the owner-kind policy's explicit next action;
9. P086 admission binds the immutable secret-safe resume-context ID/digest,
   private opaque provider-session ref, pre-session generation, spawn-pending
   process intent, owner binding, and one append-only resurrection window with
   boot-session identity and continuous-time setup/setup-cleanup deadlines before broker
   I/O; attachment consumes only the setup deadline through final prompt CAS, reserves
   the setup-cleanup interval only for pre-prompt zero-send settlement/identity-safe reap, launches/binds identity before
   proving advertised resume capability, reopens byte-equal root authorities,
   sets cwd by held descriptor, sends only inherited descriptor aliases plus the exact MCP
   inputs, rejects every pre-response authority update without reordering,
   seeds only from the correlated non-empty response catalog, orders later
   updates, reverifies the pair, identity-safely reaps every post-launch
    failure, reconciles configuration settlement before any process action and
    before choosing failure versus retained receipt/readiness plus typed post-
    configuration outcome, bounds prompt permit/write/flush/final CAS by
    `min(setup deadline, write start + 10 seconds)`, then uses only the frozen
    ordinary 300/900-second execution watchdog for terminal response; it closes
    first fatal if pre-prompt setup cleanup cannot settle by its bound and sends
   only the frozen attach branch: Claude exactly one `session/new` with
   `params.resumeSessionId`; Codex and every unsupported/wrong-capability
   permutations no attach method or prompt; `session/load`, generic fallback,
   and a second attach are always forbidden; post-expiry restart performs cleanup/settlement only
   and never creates another window; sleep and wall jumps preserve the continuous
   deadline, while reboot or missing legacy monotonic evidence permits cleanup
   only; a sentinel raw session ID is present only
   in the private map/resolver and the explicitly authorized pre-existing raw-ID
   compatibility projections, while new authority/log/error evidence excludes
   it and no raw-ID field ever contains `psref_*`; before spawn, launch copies
   the descriptor-opened bounded native image or interpreter/script closure into
   the private immutable launch directory, reopens and byte-verifies its complete
   identity, and only then assembles root/secret descriptors; post-spawn
   `proc_pidpath`/PID/start/private-inode binding is required before initialize.
   Symlink retarget, hardlink/mount replacement, binary mutation, PID reuse,
   oversized closure, or pre-spawn mismatch yields zero child instructions and
   zero credentials, while a post-spawn process mismatch reaps only an identity-
   matched child;
10. original success followed by both P079 repair kinds proves independent turns,
    source physical generation A plus a separately Seatbelt-contained attached
    repair generation B with the same logical provider-session identity, immutable
    original terminal-turn truth under repair-time invalidation, the closed
    prompt-to-configuration-owner mapping, typed
    `OutputContractRepair` work items, one logical budget, bounded zero-send
    attempt leases, TTL settlement, and atomic child creation;
11. `p079.provider_fallback_child` carries the typed provenance, owner kind,
    operation/attempt/lease authority, source occurrence, target binding digest,
    and initial one-generation permit; collateral loss allows zero transparent
    fresh sessions, attaches, or replays;
12. P086 admission atomically creates command journal, continuation/context/
    immutable resurrection window/clock, work item, turn, reserved side effect, active attempt, pre-session
    generation, process intent, and owner binding; every insert fault rolls back
    the whole tuple, timeout reconciliation occurs before launch, expiry before
    and after launch settles the same tuple, and idempotent replay returns the
    same IDs/window/clock without allocation; output-only conversion proves the
    source turn/side effect/binding zero-send, appends an immutable cause plus
    the class-appropriate typed `*_superseded_for_resurrection` post-readiness
    outcome, terminalizes that source without rebinding it, allocates a replacement
    output-only turn/side effect plus resurrection-attached generation tuple through the same private writer
    inside its registered Class A operation, appends exactly the continuation's
    first window, and replays the complete old/new
    `P086ResurrectionConversionResultV1` without another allocation or clock
    rewrite; cancellation on each side of commit targets only the authoritative
    tuple; repeated post-cleanup-expiry restarts use the five-second, 32-row,
    1-MiB cursor/checkpoint reducer and converge failed-closed with zero provider
    work or rescanning of completed keys;
13. all P086 modes bind target execution/occurrence, use their own configuration
    reservation/receipt, mirror side effects only after prompt CAS, reject fresh
    fallback, and reproduce the exact ten-value V1 phase set with an independent
    oracle; generated compatibility tests project all thirteen V2 phases to V1,
    reject direct internal-string serialization, and prove restart reconciliation
    advances only by global `reconciliation_sequence`, never opaque boot UUID;
14. sealed `steward_analysis.claim` alone creates system turn `0`, and the
    Steward executor can only load that committed turn and has no allocation
    permit; only the sealed initial-turn reservation permit may create its one
    generation after claim, including claim-before-reservation crash/race replay;
    a configured receipt followed by transport loss before dispatch terminalizes
    the lane without retry. Sealed `steward_auditor_lane.activate` alone creates auditor turn `0`, and sealed
    `steward_lane.retry_zero_send` alone preserves a terminal zero-send turn `0`
    while allocating turn `1`; the generic allocator requires an unforgeable
    permit, a second retry is rejected, and cancellation before dispatch,
    during `dispatch_pending`, and after `prompt_sent` produces three distinct
    immutable turn/lane outcomes;
15. every prompt CAS committed result
    `Applied|AlreadyMatching|Conflict|Missing` crosses every
    `DbWriterAcknowledgementV1` case; known `Missing` is never confused with
    unresolved acknowledgement `Unknown`;
16. every registered Class A operation crosses committed, busy/shutdown rejected,
    failed-before-start, uncertain-after-start, commit-before-ack, reconciliation,
    delayed commit after an empty immediate read, caller cancellation, supervisor
    takeover, writer crash, and restart for every owner kind; fixed parent JSON
    plus ordered membership rows replay a 512-owner settlement and reject
    missing/extra/reordered membership; replay remains valid after every legal
    later settlement and pointer/allocator advance, after restart, and across two
    later first-fatal/reopen cycles, while an immutable-field change, state
    regression, missing successor evidence, or unlisted successor closes fatal;
    no request-scoped task owns final reconciliation and no mutation is
    resubmitted; parent/member update/delete/late-insert negatives cover sealed
    sets of 0, 1, and 512 members, a 10,000-waiter timeout storm stays inside the
    fixed 512-permit keyed registry with terminal/fatal reserve; global result
    sequence/hash-chain, durable pending rows, and 256-row/4-MiB batches advance a
    durable high-water only after replay, the in-process 10-second and startup
    15-second continuous-time budgets fail closed without losing progress, and a
    one-million-result fixture proves no rescan at/below the checkpoint across
    repeated bounded restarts; a failed `persist_fatal` restart may reopen only
    after exhaustive durable reconciliation;
17. a manager task retains the non-cloneable generation guard through terminal
    response, receipt, active-owner/collateral settlement, and cleanup; compile
    and call-graph tests prove no coordinator second-settlement or callback cycle;
18. a transport that never completes write/flush and every broker/toolchain/
    authority/cleanup timeout remain bounded; registry admission overload is a
    typed rejected-before-start outcome while terminal/fatal reconciliation
    remains admitted; no public raw close/kill/cancel or prompt API bypass exists;
19. Claude, Gemini, Auggie, and Junie persist correlated provider-neutral
    readiness, advance the shared prompt ledger, keep accepted-pair configuration
    truth non-applicable, and fail startup without requiring a Codex receipt;
20. two owners sharing one generation cross cancellation before/after permit,
    owner interruption, generation closure, and every ordinary/P017/P058/P079
    repair/P079 fallback/P086/Steward collateral row with exact session, attach,
    replay, and prompt counts; P017 uses its stage-less mediation execution,
    mediation record, work item, and captured run epoch, and tests dispatch,
    run-cancellation, unknown delivery, restart, and wrong direct-SQL joins;
21. launch-barrier/process-binding crashes at every boundary cover original,
    P079 repair/fallback, all continuations, and both Steward lanes; only
    identity-matched processes are reaped;
22. run-wide and scoped invalidation races prove epoch/token fencing and that the
    active manager, not the invalidation caller, performs terminal settlement;
23. real three-process file-DB startup proves the four outer lock outcomes over
    one persistent device/inode without false guard ownership, pauses A during
    guard drop while B/C contend, rejects every unlink/replace path, proves only
    one successor acquires and only `acquired` binds the starting listener, and the
    inner ready/preflight-failed owner retains the one guard; before that result,
    all three may only read an existing principal file and none may create,
    chmod, repair, watch, or bootstrap it. The winner's lock-bound
    `PrincipalBootstrapOwnerPermitV1` alone performs one principal mutation and
    credential creation; losers leave an initially absent auth directory absent.
    A synthetic 101
    side effect remains absent through every migration-100 Rust-finalizer crash
    and executes exactly once after phase completion; a second clean restart
    skips the filtered source, validates the complete 101+ ledger, and changes no
    side effect, while immutable source
    snapshot/fence, full legacy mapping, process reconciliation, and consumer
    closure hold; failed preflight has no pool/in-process retry and only restart
    may recover; a ledger with migration 101 applied before phase completion is
    rejected on two restarts as
    `provider_truth_future_migration_applied_before_phase_complete` with no
    automatic adoption, rollback, or finalizer;
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
    exactly, assigns `dangling_fallback_child` independently of lease/parent
    failures, proves separate per-source canonical-plus-quarantine count/digest
    equality and disjoint keys, and never drops or merges history; shuffled
    cohorts of one/two/many same-kind active leases prove positive/ambiguous I/O
    yields no dispatch authority, while an all-zero-send cohort selects at most
    one deterministic `(lease_acquired_at, lease_key)` winner, settles losers
    `superseded_ignored`, preserves a dangling child in quarantine, and replays
    the same immutable authority after restart despite native-only
    `(repair_event_id, selected_kind)` uniqueness; all six internal lease states
    pass through the generated V1 compatibility projection on GraphQL, MCP, and
    reports, preserve old rows byte-for-byte, and reject direct enum-string or
    wildcard serialization; clean and migration-095 databases produce the exact
    checked-in normalized `P079SchemaManifestV1`, while removal or rewriting of
    every declared table/index/trigger fails startup before a selector runs;
30. direct-SQL P079 negatives reject active zero selected budget, contradictory
    flags/provenance/source schema, budget mutation, duplicate/conflicting native
    `(repair_event_id, selected_kind)` admission, wrong work-item kind,
    cross-operation turn/lease references, every forbidden update, every child-
    first/parent-first delete, and every terminal lease/operation rewrite; each
    `cancelled|superseded_ignored` no-candidate path requires immutable zero-entry
    staging/process evidence, exact empty membership digest, one zero-member set,
    and DB-only `complete_no_candidate`; replay is idempotent and every attempt to
    use an empty set for accepted/rejected or create history/activation fails. Each
    artifact-bearing validation outcome first binds provider-writable paths only
    inside the operation staging directory, rejects canonical/history writes and
    extra links, commits one prepared artifact set with its complete ordered
    members and parent held, then crash injection around every member
    write/fsync, aggregate prepare, unique history rename/fsync,
    canonical temp/clone-or-copy/rename, activation-history append/current-pointer
    CAS, member/set `history_committed|destination_committed`, and completion
    proves deterministic complete-set publish/quarantine recovery. A later repair
    reuses the same canonical path through a new immutable history member and
    advances exactly one activation revision; a lost activation CAS preserves
    both histories, blocks release, and restores the winning bytes. No duplicate/
    missing artifact, hard-link alias, or early parent release is permitted. The
    executable completion order settles operation, members, set, event, and
    parent without violating either terminal guard; every reversed/partial SQL
    order fails. Startup reconciliation checkpoints monotonic settlement/member/
    phase cursors and proves 10-second, 16-set, 256-member, and 8-MiB caps plus
    exact no-rescan resume across all boundary fixtures. Real Seatbelt fixtures
    prove candidate/source writes succeed only inside staging, bounded adapter
    state writes stay inside the separate private-state root, and canonical/
    history/workspace, symlink, hardlink, rename, and `openat` escape paths fail
    without relying on an ACP permission callback;
31. direct-SQL P058 negatives reject every cross-ledger/run/stage/agent/tier/
    attempt/policy tuple plus all update/delete attempts against immutable prompt
    authority;
32. fresh `AppSchema::sdl()` byte-matches the checked-in snapshot, every new enum
    literal is lowercase snake case, and uppercase/mixed/unknown values fail;
    both exact
    schema-version literals, simultaneous original-receipt-A/P079-repair-
    receipt-B on separately contained generation B and target-generation-A/P086-
    continuation-generation-B turn truth, and the exact production schema
    probe, `P031RunDetail`, `P031RunStageTopologyPage`,
    `P031OccurrenceExecutionAttemptPage`, `P031RuntimeTimelineSnapshot`,
    `P031RuntimeStatusChanged`, and `P031TimelineRawDetail` snapshots execute
    against real transports, resolvers, and decoders; the SDL freezes the exact
    additive `durableAfterCursor` subscription argument, snapshot schema literal,
    cursor-mode errors, strict malformed-`runId` rejection before receiver/DB
    work, the complete HTTP/WebSocket `timeline_graphql_error_v1` code matrix,
    and old-client live-only coexistence, while the closed
    prompt-turn failure enum rejects unknown strings; the SDL contains no new
    `providerSessionRefId` or public `psref_*` field, retains the existing P046
    `SessionGeneration.providerSessionRef` field byte-for-byte, and preserves
    existing authorized raw session fields;
33. same-agent interleaved execution events preserve execution, occurrence,
    sequence, presentation-row, event, lane, and per-lane human event ordinal through DB, GraphQL,
    Swift, filtering, expand/collapse, copy-ID/copy-raw, and swapped-handle
    rejection; stale toggle targets fail, ID and full/retained raw clipboard bytes
    are byte-distinct and exact; v2
    execution/active-agent/matched-event and identity-bearing raw-detail rows
    require the occurrence join, authorized unassociated events use only the
    run-events lane, and missing/unauthorized raw detail carries null identity
    without attempting a join; the authorized durable snapshot freezes bounded
    pages, publishes the newest page, persists and resolves either the event
    cursor or a durable empty-snapshot anchor before response, subscribes immediately without waiting for
    older history, lazily loads older pages, deduplicates overlap, and refetches
    only a new Timeline generation on gap/overflow; 512-event/8-MiB aggregate
    and 256-event/1-MiB live caps evict only resumable non-visible history and
    never reset run/topology/selection/focus state; raw-detail-only legacy rows
    produce immutable `legacy_gap` readback and the fixed non-retry earlier-
    activity row rather than fabricated events; completed-run, empty-anchor
    restart/race, and first/last-live-event fixtures are retained. Lane inventory covers no-data empty, zero-event
    occurrence, run-events-only, and both lane kinds, with run-events emitted iff
    an unassociated event exists;
34. historical and v2 event fixtures preserve the old `runtime_event_id`
    output for identical old inputs, while every `available`, `missing`,
    `stale`, `unauthorized`, `unavailable`, and `digest_mismatch` raw-detail
    result obeys the exact raw/error/identity nullability matrix before its
    conditional occurrence/run-events/no-identity branch;
35. topology association and layout fixtures cover all source kinds, legacy
    unique/ambiguous, 128-row occurrence/256-row transition pages under one
    immutable topology snapshot, 32-row occurrence-attempt pages, transition
    identity, SCC/median/track/virtual/self-loop rules, shuffled input, stress,
    mixed heights, field-level topology decode failure that preserves valid
    non-topology run detail, and window-owned topology recovery from every seven
    primary destinations plus Run Inspector; exact/plus-one row and byte caps
    prove bounded legacy rejection and `projection_size_limit_exceeded` with
    a noninteractive limit-only state. All four ordinary projection failures use a
    router/target/load-qualified restart-daemon action and new load generation,
    while invalid frozen labels use only the exact replacement-run guidance and
    can never invoke restart or mutate the original snapshot;
36. formatter goldens cover every legal configuration/evidence/delivery state,
    including option invalidation, every known/unknown model and effort,
    byte-distinct cancellation before acceptance versus after verified
    acceptance, every exact failure/repair/continuation/historical string,
    canonical ASCII ordinals under multiple host locales, compact bounds,
    byte-exact legal Help/copy, configured-missing, legacy-ambiguity-count, and
    generic-illegal outputs, plus mutation-negative tuple rejection; unknown
    UUID/digest/session/ref/request-shaped values use the exact bounded escaped
    or full-64-hex diagnostic representation in full/copy, hash in compact
    output, and map only to `Custom provider|model|effort` in spoken output;
37. the single `RunOccurrenceSelectionReducer` covers run change, planned-to-
    current replacement, retained previous-row metadata, next/preceding/heading
    fallback, tagged semantic occurrence/run-events/loading/failed/topology-
    unavailable/empty
    selection, persistent run-events selection across occurrence churn, and
    injective event-row/expand/collapse/copy-ID/copy-raw targets across surfaces;
    exact `presentation_target_v1` known-answer vectors cover every subject and
    control; the tagged primary/auxiliary focus-intent union rejects every
    cross-branch destination; all seven navigation destinations include explicit unregistered
    states for Artifacts/Approvals/Reports/System, while independently mounted
    primary and Run Inspector slots retain separate inventories. A stable random
    per-window routing ID plus focused-scene command router proves app commands
    and deep links mutate exactly one of two windows and use no process-wide
    notification broadcast. Epoch 0, post-unmount tombstoned exact successor,
    byte-identical same-epoch replay, lower/gap/mutated replay, and
    exact `presentation_surface_mount_v1`/`presentation_mount_v1` vectors are
    frozen; stale mount, unmount, action, focus gain, or source-qualified focus
    loss cannot affect the other slot. Inventory publication never steals
    focus; only an exact user navigation activation may transfer it, and all
    seven destinations have deterministic focus entry. The bidirectional MainActor focus bridge
    proves user-to-reducer and reducer-to-`@FocusState` changes without feedback loops;
    failed-load retry is a window-owned generation-qualified action available
    from all seven destinations and Inspector and creates one new load; Timeline
    initial loading/failure/retry, legacy gap, and
    `load_older|loading_older|retry_load_older` targets bind exact generation,
    request, source, and cursor digests and reject stale callbacks; Inspector
    `load_attempts_older|loading_attempts_older|retry_attempts_older` targets bind
    the exact selected row, topology snapshot, attempt-page generation, and page
    cursor. A background row removal clears a disappearing focused registration
    without focusing the remembered stage heading or constructing a navigation
    intent;
    the generation-qualified
    publication owner exposes loading before data, loaded empty only after
    successful available topology, matching failed/topology-unavailable state on
    error, retains the matching focusable load/topology window recovery target across registered and
    unregistered tabs, recovers restartable topology only through a new
    generation, routes frozen-input replacement through one typed
    `FrozenInputRepairRouteV1` and the same-window app-shell coordinator, keeps
    oversized topology in its noninteractive limit-only state, and drops delayed run-A responses,
    errors, focus callbacks, and updates after run B without any state mutation;
    no view-local agent-ID selection or `activeTimelineAgents.first` remains;
38. accessibility fixtures exhaust every legal subject/control matrix cell,
    include the exact human planned/sequenced/legacy discriminator only on
    occurrence controls, use exact ordinal-bearing event discriminator for
    event/lane expand/collapse/copy-ID/copy-raw, stage-heading, run-events,
    generation-qualified run and Timeline initial loading/failed/retry,
    Timeline legacy-gap, Inspector attempt-page load/loading/retry, and
    topology/restart-or-replacement, empty,
    popover, and close labels, reject incompatible pairs, contain no unknown raw
    generated provider/session/request/ref identity in spoken strings, and keep
    repeated tasks, duplicate stage titles, and same-title events byte-distinct
    through exact `HumanStageDiscriminatorV1`, task/occurrence ordinal, and lane
    event ordinal; valid human-authored event/task/stage strings that merely resemble a
    UUID/digest remain unchanged, bounded-control negatives fail or use the
    exact event fallback, and `legacyOrderUnverified` has exact visible, Help,
    and spoken output; the exact 16-case `920 x 620|1440 x 900` x
    `collapsed|expanded(280 pt)` sidebar x Inspector x
    `.large|.accessibility3` matrix proves the new production `920 x 620`
    content minimum, reachability, and no
    overlap/clipping; hosted
    `NSHostingView` tests assert actual `.focused` binding/first responder, not
    only reducer state and not physical keyboard or VoiceOver event delivery;
39. one prebound tri-state router owns the listener once after outer lock
    acquisition; two fatal/restart cycles prove global barrier-before-SQLite
    order without deadlock, one immutable fatal-cycle result per cycle, paused
    writer rollback, persisted-before-notify failure, and append-only reopen
    reconciliation; the zero-DB minimal GraphQL handler accepts only the exact
    production body/AST `P031DaemonStatus { daemonStatus { json } }` with empty
    variables and rejects every extra/missing key, selection, alias, fragment,
    directive, variable definition, batch, or unauthorized principal; the
    shipped `DaemonLifecycleClient` has no unnamed/raw alternative, polls both
    starting and failed through `P031URLSessionGraphQLReadTransport`, and rejects
    revoked, disabled, or re-scoped principals after live reload;
40. `codex-model-truth` executes the exact shared function/test-array content of
    `proposal-027`, `proposal-058`, `proposal-075`, `proposal-079`, and
    `proposal-086` without recursive shell invocation; the composed gate and
    all five standalone aliases pass on one clean recorded `HEAD`; and
41. structural mutation tests independently remove an operation registration or
    result codec, bypass the private generation writer, weaken a successor graph,
    break the Class A sequence chain/checkpoint or bounded batch, rebind an
    output-only source turn, replace a P086 boot/continuous clock with wall time,
    unlink/replace the bootstrap lock, run the filtered migrator after complete,
    broaden native P079 uniqueness, remove migrated active-authority arbitration,
    schema-manifest parity, no-candidate witness, artifact staging OS containment,
    inode-independent activation, bounded startup cursor, compatibility projection,
    executable completion order, or an artifact-set member/delete guard, omit a
    provider-session correlation/root-authority field, pre-spawn private-launch
    verification, global reconciliation sequence,
    resurrection-phase projection, closed prompt-failure enum, reservation uniqueness, readiness
    settlement codec/class-tagged permit, option-update parser, P079 provenance
    join, P058 immutable key, P017 occurrence-free envelope branch, occurrence
    copy check, production schema-probe or daemon-status GraphQL selection,
    exact durable Timeline SDL/document/event-or-empty-anchor cursor handoff,
    honest legacy-gap evidence, strict run-ID/error decoding, bounded paged
    topology/attempt document, immediate subscription, client memory cap, or
    conditional lane/event-ordinal/action field, bounded
    escaped diagnostic mapping, safe spoken mapping, bounded human-label validator, human
    stage discriminator/legacy-order suffix, per-window publication owner,
    focused per-window command router, tagged focus intent, concurrent slot
    inventory, tombstoned exact mount successor, user-activation focus guard,
    source-qualified focus loss, Timeline older-page control, all-destination
    run-load retry, or fenced topology-unavailable restart/replacement/limit-only selection/recovery
    action, or scope exclusion, and require the
    owning gate leg to fail.
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
      enqueue and claim cannot allocate again. P017 alone uses the sealed
      occurrence-free mediation branch with explicit null occurrence/stage and
      no allocator/hash call; every other producer remains occurrence-bound.
- [ ] New-generation and existing-generation reservations are separate Class A
      operations. One prompt turn has one globally unique generation binding,
      existing reuse consumes exactly one owner configuration-attempt but does not
      allocate a physical generation, idempotent replay consumes no new index,
      and every race loser performs zero provider/process/prompt I/O. Binding
      state enforces the exhaustive provider-contract-specific
      pending/receipt/readiness/failure/cancellation/post-outcome matrix and exact
      owner/attempt/generation correlation, including stage-less P017 and
      lease-owned P079 repair. Exact receipt and non-exact readiness are separate
      registered Class A settlements and produce class-tagged non-transferable
      dispatch permits for every provider-class x owner-kind cell.
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
      crash/restart. Variable collateral sets use ordered child membership rows
      plus fixed parent count/digest; a 512-owner replay cannot exceed the 16 KiB
      parent result or omit/reorder an owner. Parent/member update/delete and
      post-seal insert are rejected for 0/1/512 members. The keyed reconciliation
      registry has a fixed 512 permits split into admission, terminal/cleanup,
      and fatal reserves; coalesced overload tests prove admission cannot starve
      settlement. Replay verifies immutable witnesses
      plus the operation's generated closed successor graph, remains valid after
      later legal settlement/pointer advance/restart and two later fatal cycles,
      and closes fatal on regression, missing evidence, or an unlisted successor.
      Failure/cancellation/post-readiness evidence is immutable; cleanup
      progression uses separate append-only events and a terminal reduction.
      Results carry one contiguous append-only sequence/hash chain; durable
      pending envelopes and a high-water checkpoint bound startup to 256 rows,
      4 MiB, and 15 continuous-clock seconds per attempt. The in-process
      supervisor is bounded to 10 seconds; P086 further clamps it to the
      immutable cleanup deadline and transfers the durable envelope to startup
      with no detached task. A one-million-result fixture
      proves constant-memory progress across restarts with no scan at or below
      the durable checkpoint.
- [ ] One `FirstFatalCoordinator` is the sole close authority. It closes prompt
      and mutation admission under the global barrier-before-SQLite writer order,
      durably persists exactly one immutable fatal-cycle result, and only then
      publishes failed state; no competing CAS/watch owner or lock inversion
      exists. Restart appends reconciliation and opens a new epoch/cycle without
      rewriting old evidence. The outer mode-0600 lock path retains one stable
      inode across normal drops; no production path unlinks/replaces it, and a
      three-process race proves one successor guard. Only outer lock `acquired`
      binds one prebound router; inner bootstrap transitions `starting -> normal|failed` and
      `normal -> failed`, while failed bootstrap retains its guard without a pool
      or in-process retry. Before lock acquisition, principal handling is read-
      existing-only and cannot create/chmod/repair/watch; only the lock-bound
      `PrincipalBootstrapOwnerPermitV1` may bootstrap or mutate credentials.
      Three-process empty-auth-root proof yields one owner mutation and no loser
      filesystem change. The zero-DB Operator GraphQL exception accepts exactly
      the shipped `P031DaemonStatus` body, empty variables, and
      `daemonStatus { json }` AST. The shipped `DaemonLifecycleClient` uses that
      named production transport for starting/failed polling and live principal
      revocation/disable/scope changes. `persist_fatal` failure cannot reopen
      service in-process; a later restart reopens only after exhaustive durable
      reconciliation proves the prior mutation absent or terminal.
- [ ] Session lineage, generation, process binding, launch barrier, PID/start
      identity, boot session, parent daemon identity, and bounded cleanup support
      run-agent, P086-continuation, and Steward-lane owners. Same-parent cleanup
      alone may reap with `waitpid`; restart uses verified termination/absence
      and never signals an identity-ambiguous or PID-reused process. Before
      spawn, the bounded native-image or declared interpreter/script closure is
      opened without symlinks, copied from descriptors into a private daemon-
      owned launch directory, made immutable to the daemon, and fully reverified.
      Root/secret descriptors are assembled only after that final verification.
      After spawn, PID/start/`proc_pidpath`/private-inode binding is required
      before initialize; restart trusts only the persisted private image.
      Retarget, hardlink/mount replacement, binary mutation, oversize, or any
      tuple mismatch fails closed with zero child credentials/instructions or
      identity-matched cleanup only.
- [ ] `provider_prompt_turns`, not terminal receipts or owner-domain rows, is
      sole dispatch authority. Initial/final CAS, byte certainty, unknown
      quarantine, cancellation, and restart cover ordinary, P017, P058, P079
      repair/fallback, P086, and Steward prompts. P017 is explicitly stage-less:
      its authority joins the mediation execution/record, work item, prompt owner,
      and captured run epoch, and rejects a fabricated stage tuple.
- [ ] Migration 095 remains byte-identical. New migration 100 owns complete P079
      v2 DDL and exact 095-source mapping, stages/restarts without row loss or
      merging, routes dangling mandatory identities including fallback-child
      mismatch to typed quarantine with independent per-source
      canonical-plus-quarantine count/digest proof, and enforces unique logical
      admission only for `native_v2`, while preserving multiple valid same-kind
      migration-095 leases as distinct operations. A complete cohort reducer
      creates at most one immutable active authority for a deterministic eligible
      zero-send winner; positive/ambiguous I/O creates none and zero-send losers
      settle `superseded_ignored`. Selected budget, provenance,
      source schema/key, work item, turn,
      operation, attempt, terminal immutability, active state, and timestamps
      through direct-SQL negatives. An incomplete phase runs the filtered
      embedded `<=100` source, Rust finalization, then the full source; a complete
      phase skips the filtered source and validates the full ledger. Synthetic
      migration 101 cannot run early; if already recorded before phase completion
      startup refuses as operator corruption on every restart, while a legitimate
      second clean restart after 101 is byte-stable. The sole generated P079 V1
      compatibility projection covers every v2 lease state on GraphQL/MCP/report
      readback; direct enum strings, wildcard fallback, and unlisted states fail.
      `P079SchemaManifestV1` byte-proves every table/index/trigger, including
      validation evidence, no-candidate witness, settlement allocator,
      reconciliation checkpoint, migration quarantine, and active authority,
      from empty and migration-095 databases; installed-schema drift fails before
      a P079 selector runs.
- [ ] P079 repair uses `OutputContractRepair`; its fallback child remains an
      `InvokeAgent` item but carries typed `production.p079_fallback`
      provenance and `p079_fallback_child` prompt ownership. Its initial permit
      joins operation/attempt/lease/parent/binding authority, and collateral loss
      permits zero transparent fresh session, attach, or replay. Original and
      repair turns use source physical generation A and a separately Seatbelt-
      contained generation B attached through the frozen provider-specific
      protocol to the same logical provider-session identity. The repair target
      turn must be `not_started`, preserving the sent original turn. Repair and
      fallback both require `macos_seatbelt_staging_v1`; provider/descendant
      candidate or source mutations outside the descriptor-bound staging root
      fail at the OS boundary, while bounded adapter state is confined to a
      separate non-output private root. This holds even without an ACP permission request, and unsupported/advisory-
      only containment is zero-send. The parent execution's
      current receipt pointer remains receipt A permanently; receipt B is
      reachable only through the repair lease/attempt binding and repair-turn
      child, and no reducer may project B onto the parent. Validation settlement
      gives the provider only deterministic operation-staging output paths and
      denies canonical/history roots; runtime validates those dirfd-bound members
      before it closes item/lease and writes one prepared artifact set plus its complete
      ordered required-output members while parent/transition stay held.
      Cancellation/supersession with no candidate requires immutable zero-entry
      staging/process evidence and completes one constrained zero-member set
      through DB-only `complete_no_candidate`; accepted/rejected outcomes can
      never use that path. The
      daemon reconciler commits each candidate to an immutable unique history
      path, atomically installs canonical bytes for accepted outputs, and advances
      the `(run, canonical path)` activation pointer by expected-revision CAS;
      canonical publication uses only an inode-independent clone or bounded copy,
      never a hard link; quarantine history needs no activation. A later repair may name the same
      canonical path through a new history member, but exactly one activation is
      current. Separate registered history-member and destination-member Class A
      operations own each filesystem/activation mutation. The final Class A
      completion performs no filesystem or activation write and settles
      set/members/operation/event/parent only after every member result and
      destination durability agree, in the exact trigger-compatible order.
      Artifact-set canonical paths are unique.
      Delete,
      immutable-field, direct activation, and stale-revision rewrites are rejected.
      Crash at every boundary converges without missing/duplicate publication or
      early release; a lost activation CAS preserves history and keeps the parent
      blocked. Startup settlement is cursor-addressed and bounded to 10 seconds,
      16 sets, 256 members, and 8 MiB per attempt, resumes from the last durable
      member/phase, and never rescans a completed sequence.
- [ ] P058 prompt authority is an immutable complete
      execution/ledger/run/stage/agent/tier/kind/attempt/policy tuple. Composite
      FK plus insert/update/delete negatives prevent cross-ledger use or
      post-reservation tier mutation.
- [ ] P086 admission atomically commits command, continuation/context, one
      immutable resurrection window with boot-session identity and continuous-
      time setup/setup-cleanup authority,
      item, turn, side effect, active attempt, pre-session generation,
      spawn-pending process intent, and owner binding before broker I/O. It uses
      one persisted 30-second setup duration through acquisition, launch,
      initialize, resume, reverify, prompt dispatch, and final sent CAS, followed
      by a distinct 10-second pre-prompt cleanup-only duration on the same boot.
      After `prompt_sent`, the ordinary frozen 300/900-second execution watchdog
      exclusively bounds terminal response and its own terminal cleanup; the
      setup clock cannot shorten it. Sleep and wall-clock jumps cannot extend
      setup, and reboot makes only unprompted setup work expired. Restart/replay allocates
      nothing and cleanup expiry closes first fatal. Post-expiry, post-reboot, or
      legacy-window-unverifiable startup may only identity-check/reap/settle
      the same tuple and cannot broker, spawn, resume, configure, prompt, or
      replace the window. `GenerationReservationWriterV1` is the sole private
      tuple constructor; its only enclosing callers are exactly
      `provider_configuration.reserve_new_generation`,
      `p079_repair.admit_or_retry`, `p079_fallback.admit_or_retry`,
      `p086_continuation.admit`, and
      `p086_continuation.convert_output_only_to_resurrection`; live/existing
      reuse never invokes it. Prompt permit, transport write, flush, and final
      CAS share the immutable deadline
      `min(setup_deadline_continuous_ns, write_start_continuous_ns + 10 seconds)`;
      cleanup never grants another write interval. Output-only conversion proves
      and terminalizes the old turn/side-effect/binding as zero-send, appends an
      immutable cause plus the class-appropriate typed supersession outcome, allocates a
      replacement output-only turn with a resurrection-attached tuple, appends exactly its first window,
      and replay returns the same complete old/new clock-bound result. Cleanup
      reconciles the exact configuration journal before any process action or
      evidence settlement: committed receipt/readiness is retained with a typed
      post-configuration outcome, committed failure is replayed, and uncertainty
      is failed-serve. Same-parent cleanup alone may `waitpid`; restart verifies
      boot session, parent, PID, process-start identity, executable, and process
      group before termination/absence settlement. Post-cleanup-expiry startup
      uses the durable five-second, 32-row, 1-MiB ordered checkpoint reducer and
      never rescans completed keys or performs provider work; windows advance by
      monotonic `reconciliation_sequence`, never boot UUID lexical order.
      `ResurrectionPhaseV1` remains the exact ten-value public/migration contract,
      while all thirteen internal V2 values pass through the sole generated V1
      projection. Historical
      `AcpRuntimeReceipt.provider_session_id` and existing authorized GraphQL/MCP/
      report semantics and bytes remain unchanged; P046
      `SessionGeneration.providerSessionRef` retains its exact 32-hex process-
      salted derivation, fixed-salt bytes, authorization, and restart instability.
      New P086 runtime receipts use
      the same raw-or-null namespace and never place `psref_*` there. Private
      authority uses relational `provider_session_ref_id`, which is absent from
      every northbound/Swift schema. The exhaustive generated correlation
      manifest migrates every schema/call-site correlation, while raw wire or
      legacy projection access requires the purpose-limited zeroizing resolver.
      Resurrection binds process identity before checking advertised capability,
      reopens byte-equal descriptor/inode/mount-bound root authorities under the
      mandatory syscall-enforced profile, and uses only the frozen supported
      provider-specific attach branch: Claude
      `session/new.params.resumeSessionId`. Codex remains zero-send unsupported
      until a later pinned conformance manifest/version authorizes it. It rejects
      pre-response option updates, and requires a correlated complete response
      catalog. Every post-launch failure reaps by identity; every mode owns
      configuration evidence, preserves target occurrence, rejects fresh
      fallback, and passes the old phase/release oracle. Prompt-turn terminal
      failures use only `ProviderPromptTurnFailureCodeV1` and its exact
      state/nullability matrix; unknown strings fail decoding.
- [ ] Steward persists analysis and both internal lane owners before provider
      I/O, uses no synthetic run/execution, and exposes only sealed analysis-claim,
      auditor-activation, and zero-send-retry constructors over the private turn
      allocator. Claim alone creates system turn `0`; the executor only loads it
      and has no allocation permit; the sealed initial-turn reservation permit
      creates exactly one generation after claim, including claim-before-
      reservation replay. A committed configured receipt followed by transport
      loss before dispatch terminalizes that lane and cannot satisfy zero-send
      retry preconditions. It applies one total delivery reducer including
      `steward_pending|steward_sent|unknown`, preserves failed turn `0` before
      allocating retry turn `1`, and distinguishes cancellation before dispatch,
      during dispatch, and after prompt sent. Both lanes settle with at most one
      durable zero-send retry.
- [ ] The closed collateral matrix covers ordinary, P017, P058, P079 repair,
      P079 fallback, P086, and Steward. A sent collateral turn is never replayed;
      only the explicit ordinary policy or Steward's durable zero-send
      turn-0-to-turn-1 retry CAS may allocate a fresh generation after closure;
      P079/P086 never may.
- [ ] Startup keeps consumers closed until migration, receipt/invalidation,
      process, turn, quarantine, and owner reconciliation complete. Generated
      replay selectors reject every unresolved or unclassified path. Duplicate,
      anomalous, and failed lock outcomes never bind/open; only the acquired
      guard enters phased preflight and can publish a router.
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
      `ProviderExecutionTruthSchemaProbe`, `P031RunDetail`,
      `P031RunStageTopologyPage`, `P031OccurrenceExecutionAttemptPage`,
      `P031RuntimeTimelineSnapshot`, `P031RuntimeStatusChanged`,
      `P031TimelineRawDetail`, and `P031DaemonStatus` operations from the exact
      shipped `P031GraphQLDocumentSet` properties contain their complete required
      selections. Run-detail requests bounded non-topology truth; the two paged
      topology operations request every occurrence/transition/attempt field under
      exact row/byte/snapshot caps. Shipped DTOs distinguish omission from null
      and no query selects execution truth by agent ID. Legacy unpaged fields
      reject over-limit results before materialization.
      The additive `durableAfterCursor` argument, strict supplied-`runId` parse,
      complete `timeline_graphql_error_v1` HTTP/WebSocket decoding,
      `runtime_timeline_snapshot_v1` literal, complete named query/subscription
      documents, and old-client live-only behavior are exact. Execution and turn
      schema-version literals are exact, and turn-owned
      configuration readback simultaneously preserves original and P079/P086
      specialized generation truth. Every new enum literal is explicit lowercase
      snake case; uppercase, mixed-case, and unknown values are rejected,
      including prompt-turn failure codes. No
      `providerSessionRefId` is present; existing authorized raw session fields
      retain their prior semantics, and the existing P046 derived
      `providerSessionRef` remains byte-identical for a fixed salt and distinct
      across process salts/restarts. The production `P031DaemonStatus` document is
      named, has empty variables, and is the sole daemon lifecycle polling path.
- [ ] Existing MCP `2024-11-05`, `run://`, `report://`, `reports.get`,
      tools envelopes, `steward.list_analyses`, `steward.get_analysis`, generated
      reports, and all non-P079 artifact bytes/provider filesystem grants remain
      unchanged. P079's existing northbound lease fields use only the specified
      V1 compatibility projection, while its repair/fallback provider grant moves
      from canonical outputs to operation staging and no other lane expands.
      Structural tests reject accidental aliases or unrelated protocol/report/
      materializer/filesystem expansion in this slice.
- [ ] `AcpRuntimeReceipt` remains schema v1. Migration preserves every
      `receipt_json` byte, stores prompt/configuration correlation only in
      private relational columns/tables, and byte-compares existing Operator,
      Agent, and Observer report/MCP projections before and after migration.
- [ ] GraphQL and Swift distinguish planned, configuring, configured,
      invalidated, prompt-pending/sent/unknown, failed, cancelled, and legacy
      states. One formatter owns visual, Help, copy, and accessibility values and
      never renders invalidated or planned values as actual. Cancellation before
      acceptance and after verified acceptance have byte-distinct copy and legal
      tuple predicates. Every legal state, configured-missing acceptance,
      bounded legacy ambiguity, and generic illegal tuple has exact normative
      visual/Help/copy/accessibility bytes. Full/Help/copy use only
      `BoundedDiagnosticIdentitySegmentV1`: bounded printable text with exact
      quote/backslash/control/bidi escaping or the full 64-hex fallback for
      invalid/oversized input. Compact output uses the frozen domain hash, and
      `accessibilityIdentity` maps every unknown segment only to the closed
      `Custom provider|model|effort` literals; spoken output contains no raw,
      digest, UUID, session ID/ref, or request ID.
- [ ] Timeline and raw-detail identity preserve exact event, execution,
      occurrence, sequence, presentation-row, lane, and per-lane event-ordinal tuples through DB,
      GraphQL, shipped Swift DTOs, conditional occurrence-presentation join,
      filtering, expand/collapse, and byte-distinct copy-ID/copy-raw actions.
      Identity-bearing rows require the join;
      authorized unassociated events use run-events; `missing`/`unauthorized`
      require null identity and never attempt a join. The old `rte_` algorithm/
      handles remain byte-compatible and all six status rows obey exact
      raw/error/identity nullability. Every occurrence exposes its lane even with
      zero events; the run-events lane exists iff at least one authorized
      unassociated event exists, with exact no-data/run-only/occurrence-only/both
      fixtures. Initial load fetches the newest authorized page under a frozen
      cursor, commits/resolves an event or durable empty-snapshot handoff anchor
      before exposing it, publishes the page, registers live delivery immediately, drains and
      deduplicates the atomic cursor handoff, and loads older pages only by
      the exact generation/snapshot/page-qualified explicit control. Existing
      raw-detail-only history is represented by immutable `legacy_gap` evidence
      and `Earlier activity unavailable`; no event envelope is fabricated.
      Initial loading, retryable/non-retryable initial failure, gap refetch, and
      legacy-gap rows each publish only their exact typed, focusable Timeline
      target; retry allocates one new Timeline generation. Inspector older-
      attempt load/loading/retry publishes only its selected-row/snapshot/page-
      generation/cursor-qualified target and rejects every stale callback.
      Retention gap, overflow, sequence gap, or digest mismatch
      starts only a new Timeline generation without resetting run load, topology,
      selection, or focus. Aggregate client storage is capped at 512 events/8 MiB
      and live buffering at 256 events/1 MiB; eviction preserves visible/live
      rows and a resumable older-page cursor. Completed runs render durable
      history without a live event.
- [ ] One `RunOccurrenceSelectionReducer` owned by
      `P031RunsHomeViewModel` governs Overview, Stages, Timeline, Run Inspector,
      popover, and focus, while all seven navigation destinations are modeled and
      Artifacts/Approvals/Reports/System are explicit unregistered primary states.
      Primary and Run Inspector are independent concurrent slots, and the focus
      intent is a closed primary/auxiliary tagged union. It retains prior selected-row metadata across row-array
      replacement, represents semantic selection as occurrence/run-events/
      loading/failed/topology-unavailable/empty, forbids empty while topology is
      unavailable, preserves run-events across row churn, and uses
      injective event-row/expand/collapse/copy-ID/copy-raw targets with exact
      clipboard payloads plus failed-load and topology-recovery controls.
      Timeline initial loading/failure/retry, legacy gap, and older-page
      load/loading/retry are separate exact targets. Inspector attempt paging is
      a separate selected-row/topology-snapshot/page-generation/cursor-qualified
      state and target. Run-load and topology recovery are separate window-owned
      registrations; the former remains focusable on all seven destinations and
      Inspector, while topology size overflow offers only a noninteractive
      limit state.
      `presentation_target_v1` has one frozen hash encoding and known-answer
      corpus. Registered mounts require epoch 0 only for a new request/load,
      preserve the last accepted epoch as an unmount tombstone, require exact +1
      on same-tuple remount/successor, and allow only byte-identical same-epoch
      replay; lower/gap/mutated events fail. Exact
      `presentation_surface_mount_v1` and derived `presentation_mount_v1` tokens
      qualify every slot action. The reducer rejects stale mount/unmount/action,
      focus gain, and source-qualified focus loss without affecting the other
      slot. Inventory publication preserves surviving focus or clears it and
      never transfers focus; only a matching user-navigation activation intent
      may focus one of the seven deterministic destination roots. The
      bidirectional focus bridge
      reconciles reducer and actual `@FocusState` changes without feedback loops.
      Retry actions create a new matching load generation. A generation-qualified
      publication owner publishes loading before request, loaded empty only after
      valid available topology, matching failed or topology-unavailable state on
      error, and exposes the matching window-owned focusable recovery registration
      on all seven destinations and Inspector. Ordinary projection failures
      restart only through a new generation; `frozen_input_invalid` offers only
      the exact immutable-snapshot replacement-run flow, and
      `projection_size_limit_exceeded` exposes only the noninteractive bounded-
      limit summary. Frozen-input replacement constructs one complete
      `FrozenInputRepairRouteV1`, routes through the focused window's
      `AppShellNavigationCoordinatorV1`, opens Definitions plus replacement in
      that same window, preserves the immutable original snapshot, and is
      idempotent by operation ID and complete route tuple.
      The owner drops every stale run response,
      error, focus callback, or update without mutating the newer run. It has no
      per-run dictionary, view-local agent selection, or
      `activeTimelineAgents.first` fallback. `ContentView` creates exactly one
      per-window `P031RunsHomeViewModel`; `RunsHomeView`, ordinary run detail,
      direct/deep-link presentation, and Run Inspector receive that same object,
      and teardown cancels its sole publication/subscription lifecycle. One
      random `RunWindowRoutingIDV1` and focused-scene router address app commands
      and deep links to exactly one window; process-wide notification broadcast
      is forbidden and two-window fixtures preserve the background window.
- [ ] Accessibility implements the exhaustive subject/control matrix. Only
      occurrence controls include the exact planned, sequenced, or legacy human
      discriminator built from the unique `HumanStageDiscriminatorV1`; event
      controls use the exact per-lane human event ordinal
      to distinguish expand, collapse, copy ID, and full/retained raw copy, while
      close, Timeline older-page load/loading/retry, heading, run-events,
      generation-qualified run and Timeline initial loading/failed/retry,
      Timeline legacy gap, Inspector attempt-page load/loading/retry,
      topology/restart-or-replacement-or-limit, and empty controls use their exact discriminator-free
      strings. No spoken value contains generated provider/session/request/ref
      identity; duplicate stage titles, repeated tasks, and same-title events are
      spoken distinctly without opaque IDs, while human-authored event/task/stage
      text that resembles a UUID or digest is preserved only after
      `BoundedHumanLabelV1` validation. The 1..256-scalar/1024-byte/control and
      bidi bounds produce `frozen_input_invalid` and replacement-only recovery
      for invalid stage/task labels and use exact
      `Event` fallback for an invalid optional event title. A legacy stage emits
      exact visible `Order unverified`, exact Help, and exact spoken
      `, order unverified` suffix. Stage, source, occurrence,
      and event ordinals use locale-independent ASCII decimal;
      opaque identity remains only in machine identifiers. Pure/hosted tests
      prove every legal pair, reject illegal pairs, run the exact 16-case
      `920 x 620|1440 x 900` x `collapsed|expanded(280 pt)` sidebar x Inspector x
      `.large|.accessibility3` matrix after enforcing the production
      `920 x 620` content minimum, without overlap/clipping, and assert real `.focused`/
      first-responder targets without claiming physical keyboard/VoiceOver proof.
- [ ] `./scripts/test-gate.sh codex-model-truth` runs nonzero Rust and
      independently nonzero pure/hosted Swift suites and the exact shared legs
      from P027/P058/P075/P079/P086. The composed gate and all five standalone
      aliases pass on one clean committed `HEAD` with no relevant untracked
      implementation path.
