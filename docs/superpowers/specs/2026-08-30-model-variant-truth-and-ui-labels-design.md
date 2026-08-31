# Codex Exact Variant Dispatch and Planned UI Labels

Date: 2026-08-30
Status: Draft; decomposed active slice awaiting proposal-readiness approval
Source checkpoint: `acf85de1`

## Decision summary

This document is the only active implementation proposal in the model-truth
decomposition. It tests one bounded hypothesis:

> If fresh Codex profiles freeze an exact GPT-5.6 variant and effort, the ACP
> adapter refuses to send a prompt until that exact pair is selected, and the
> existing Overview and Stages projections render the full planned pair, then
> operators can distinguish Sol, Terra, and Luna without first building a new
> durable provider-acceptance subsystem.

The slice is intentionally limited to fresh catalog validation, fail-closed
pre-prompt configuration, and honest planned identity in the macOS shell. It
does not claim that planned values are durable provider-accepted truth.

The oversized predecessor is retained in Git at `acf85de1` for source
traceability. Its independent contracts are assigned to deferred child
documents under [Decomposition](#decomposition). Those children are source
inventories, not implementation authority, and are not part of this review
verdict.

## Current baseline

- All seven active Codex backend profiles in `examples/agents/agents.yaml` use
  generic `gpt-5.6`.
- Workflow snapshots already freeze `provider`, `model`, and `effort` for each
  resolved task.
- `runStageTopology.occurrences` already returns planned `provider`, `model`,
  and `effort` from the frozen run plan.
- Stages already combines those three values into compact operator copy, which
  is why the UI currently shows `codex · gpt-5.6 · high`.
- `activeAgentExecutions` returns `model` but not `effort`; its rows retain
  `backend_profile_id`, so effort can be projected from the same frozen catalog
  without a new persistence column.
- The Codex adapter sends a model in `session/new` and applies effort through a
  best-effort `session/set_config_option`. It does not require exact model and
  effort verification before `session/prompt`.
- Existing frozen snapshots have no compiler-owned marker that distinguishes
  a newly validated exact binding from an older snapshot that happens to
  contain the same model and effort strings.
- The app and bundled daemon normally move in lockstep, but GraphQL rejects a
  document that selects a field absent from the running daemon. Nullable Swift
  decoding alone is therefore not a compatibility boundary.

## Goals

1. Freeze one exact Codex variant and explicit effort for each fresh Codex
   backend profile.
2. Reject fresh catalogs whose Codex profile matrix is generic, unknown, or
   weaker than the approved matrix.
3. Require exact model and effort selection before the first prompt of every
   fresh exact-pair Codex invocation.
4. Send zero prompt bytes when the provider cannot prove the exact requested
   pair.
5. Show a readable variant, exact model ID, and effort in both Overview and
   Stages using planned frozen truth.
6. Preserve every pre-change frozen run byte-for-byte and label its planned
   identity as legacy/unverified rather than inferring a variant.
7. Ship the behavior enabled by default with no feature flag or disable path.

## Non-goals

- Persist or display `accepted_model`, `accepted_effort`, option-snapshot
  revisions, prompt permits, or provider-acceptance receipts.
- Reuse an exact-pair Codex physical session across separate invocations.
- Change provider-session resurrection, output-only recovery, P079 repair
  materialization, or general provider-fallback/escalation policy beyond making
  the exact terminal failure outcomes explicitly ineligible. Persisting the
  current run-local fallback decision when it selects exact Codex changes
  authority/audit shape only, not selection policy.
- Redesign Timeline, topology pagination, raw-detail readback, frozen-run
  replacement, migration bootstrap, or daemon failed-serve behavior.
- Add runtime model selection or an operator setting.
- Change Claude, Gemini, Auggie, or Junie model resolution.
- Claim that a planned model shown after restart is independently reverified
  provider truth.
- Require a live provider, network access, remote UI host, or dedicated
  Chainworks run for release evidence.

## Approved model and effort matrix

| Backend profile | Exact model | Effort | Reason |
|---|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `max` | Cross-stage authority and hard decisions |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` | Architecture and contract review |
| `codex_audit_high` | `gpt-5.6-sol` | `ultra` | Read-only final audit |
| `codex_writer_high` | `gpt-5.6-terra` | `high` | Iterative proposal authoring |
| `codex_builder_high` | `gpt-5.6-terra` | `high` | General implementation work |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `high` | Routine orchestration |
| `codex_ops_low` | `gpt-5.6-luna` | `high` | Bounded operations with the approved reasoning floor |

The historical `codex_ops_low` identifier remains stable; its frozen `effort`
value is authoritative. `low` and `medium` remain parser-supported provider
values but are not approved for an active Chainworks Codex profile. Luna with
`ultra` is invalid. No profile outside this table changes in this slice.

## Contract

### 1. Fresh catalog authority

The immutable source fixture is
`examples/agents/codex-model-variant-matrix.v1.json`. Its top-level object is
strictly limited to:

```text
schema_version = 1
policy_id = codex_model_variant_matrix_v1
provider = codex_acp
variants[] = { model_id, display_name, allowed_efforts[] }
production_profiles[] = { backend_profile_id, model_id, effort }
```

The exact UTF-8 fixture is 1,446 bytes including its final LF and has SHA-256
`b9119b3e4375d46d8e9ad29b615ca3d8385357be8f7415869e4c51590bc31395`.
That digest is normative for policy v1, not derived at runtime from whichever
fixture happens to be current.

Its bytes are exactly this JSON followed by one LF:

```json
{
  "schema_version": 1,
  "policy_id": "codex_model_variant_matrix_v1",
  "provider": "codex_acp",
  "variants": [
    {
      "model_id": "gpt-5.6-sol",
      "display_name": "Sol",
      "allowed_efforts": ["low", "medium", "high", "xhigh", "max", "ultra"]
    },
    {
      "model_id": "gpt-5.6-terra",
      "display_name": "Terra",
      "allowed_efforts": ["low", "medium", "high", "xhigh", "max", "ultra"]
    },
    {
      "model_id": "gpt-5.6-luna",
      "display_name": "Luna",
      "allowed_efforts": ["low", "medium", "high", "xhigh", "max"]
    }
  ],
  "production_profiles": [
    {
      "backend_profile_id": "codex_orchestrator_high",
      "model_id": "gpt-5.6-sol",
      "effort": "max"
    },
    {
      "backend_profile_id": "codex_architect_high",
      "model_id": "gpt-5.6-sol",
      "effort": "xhigh"
    },
    {
      "backend_profile_id": "codex_audit_high",
      "model_id": "gpt-5.6-sol",
      "effort": "ultra"
    },
    {
      "backend_profile_id": "codex_writer_high",
      "model_id": "gpt-5.6-terra",
      "effort": "high"
    },
    {
      "backend_profile_id": "codex_builder_high",
      "model_id": "gpt-5.6-terra",
      "effort": "high"
    },
    {
      "backend_profile_id": "codex_orchestrator_acp",
      "model_id": "gpt-5.6-terra",
      "effort": "high"
    },
    {
      "backend_profile_id": "codex_ops_low",
      "model_id": "gpt-5.6-luna",
      "effort": "high"
    }
  ]
}
```

The complete supported vocabulary is:

| Variant | Allowed efforts |
|---|---|
| `gpt-5.6-sol` | `low`, `medium`, `high`, `xhigh`, `max`, `ultra` |
| `gpt-5.6-terra` | `low`, `medium`, `high`, `xhigh`, `max`, `ultra` |
| `gpt-5.6-luna` | `low`, `medium`, `high`, `xhigh`, `max` |

`production_profiles` contains exactly the seven rows in the approved matrix.
Each `variants[].model_id` and each
`production_profiles[].backend_profile_id` is unique; a production model ID
must reference one declared variant. Unknown fields are rejected, and the JSON
loader rejects duplicate object keys before typed decoding. The YAML loader
likewise rejects duplicate `backend_profiles` mapping keys before catalog
decoding; last-key-wins behavior is forbidden.

The dependency-leaf module
`domain::codex_model_variant_policy` owns append-only
`CodexModelVariantPolicyRegistryV1`. It has no DB, ACP, workflow, or readback
dependency. Its v1 entry pins the policy ID, fixture path, byte length, literal
digest, supported pairs, and production-profile rows above. `workflow`, `acp`,
engine, and GraphQL import this one module rather than copying registry tables.
The compiler embeds and resolves every retained registry entry, not only the
newest one.
Neither the v1 entry nor its fixture may be replaced or removed. A vocabulary
change adds a new fixture filename, policy ID, digest, and registry entry while
retaining v1 for old snapshots. A test-only second policy proves both versions
can compile and dispatch in the same binary.

The Rust workflow compiler verifies embedded bytes against the independent
registry entry and is the sole run-creation authority. The Xcode target bundles
all retained fixtures and the same read-only registry values so `YAMLValidator`
can provide preflight feedback, but Swift does not compile or authorize a run.
Missing, malformed, removed, or digest-mismatched bundled data is a visible
validation error. The retained gate proves source bytes, bundled bytes, and
Rust-embedded bytes match the pinned registry entry and rejects mutation of an
existing policy in place.

For every fresh catalog, a Codex profile must use an exact supported variant
and a valid effort; when a profile uses one of the seven production IDs, its
pair must match the fixture. The canonical production-catalog gate additionally
requires all seven rows and no additional Codex profile. Rust compilation
fails before run creation when any of these conditions is true:

- a profile ID is duplicated;
- its provider is not `codex_acp`;
- its model or effort differs from the approved row;
- any Codex profile uses generic `gpt-5.6`;
- a Codex model is not one of `gpt-5.6-sol`, `gpt-5.6-terra`, or
  `gpt-5.6-luna`;
- an effort is outside the provider vocabulary; or
- Luna is paired with `ultra`.

The validation is unconditional for newly compiled catalogs. There is no YAML
switch, environment override, or UI preference.

The compiler writes the exact authored model and effort into the existing
frozen catalog and resolved-agent fields. It keeps outer
`catalog_snapshot_format_version = 2` and writes a versioned compiler marker:

```json
{
  "schema_version": 2,
  "model_variant_policy": {
    "schema_version": 1,
    "policy_id": "codex_model_variant_matrix_v1",
    "fixture_sha256": "b9119b3e4375d46d8e9ad29b615ca3d8385357be8f7415869e4c51590bc31395"
  }
}
```

This is the relevant subset of `chainworks_compiled`; its existing required
mission-context and skill-bundle fields retain their compiled values. The
existing schema v1 remains readable and unchanged. Schema v2 requires the
complete marker above, and `fixture_sha256` is the lowercase SHA-256 of the
exact checked-in fixture bytes. An unknown policy, malformed digest, or digest
that does not match the embedded fixture is invalid for execution and cannot
silently enter either exact or legacy behavior.

### 2. Frozen replay boundary

Runs created before this change retain their workflow/catalog snapshot bytes
and current adapter behavior. Every `chainworks_compiled` v1 snapshot is
classified as `legacy_best_effort_v0`, including one that already contains
`gpt-5.6-sol`, `gpt-5.6-terra`, or `gpt-5.6-luna`. Model strings alone never
opt an old run into new execution behavior or a friendly variant label.

The engine derives a typed `CodexConfigurationModeV1` from frozen compiler
provenance, not from caller input:

```text
LegacyBestEffortV0
ExactVariantV1 { policy_id, fixture_sha256, backend_profile_id }
```

The engine loads the frozen plan by `run_id` and resolves the expected task
through the durable `TaskOccurrenceKeyV1` defined below. A stage occurrence
supplies `backend_profile_id`; a persisted frozen escalation tier supplies it
only when the invocation was already routed through that tier for some other
failure. Queue payload values, including agent and profile IDs, are comparison
evidence only. A missing occurrence/profile or any
payload/profile/provider/model/effort mismatch rejects before child launch; a
second valid matrix pair cannot be substituted. `ExecutionRequest` carries the
resolved `backend_profile_id`, occurrence key, and typed mode, and the adapter
rechecks the pair against the pinned policy entry before `session/new`.

Every newly enqueued exact Codex invocation has a durable
`TaskOccurrenceKeyV1` before it is claimable:

```text
StaticStageTask { stage_execution_id, frozen_task_index }
DynamicStageTask {
  stage_execution_id,
  materialization_epoch,
  selection_plan_hash,
  frozen_binding_id,
  selection_index
}
LeadConflictMediation { mediation_record_id }
```

For a static stage, `frozen_task_index` is the zero-based position in the
frozen concatenated order `tasks` then `post_approval_tasks`. A dynamic key is
materialized in the same transaction as its existing P060
`DynamicMaterializationRecord`; `frozen_binding_id` must resolve one
`dynamic_candidate_bindings` entry in the frozen plan and every other field
must match the validated selection plan/materialization row. The canonical
encoded key and `attempt_number` are persisted on both the work item and each
AgentExecution attempt. They are not reconstructed from `agent_id`. Attempts
for one occurrence are ordered by
`attempt_number DESC`, then `started_at DESC`, then AgentExecution ID byte
order. Legacy rows may keep a null key and retain existing best-effort
readback; every exact-v1 row requires a key and positive attempt number.

Binding selection is separately typed as `ExecutionBindingAuthorityV1`:

```text
FrozenStaticProfile { backend_profile_id }
FrozenDynamicBinding { frozen_binding_id, backend_profile_id }
FrozenSystemLead { agent_id, backend_profile_id }
FrozenEscalationTier {
  ledger_id,
  tier_id,
  tier_attempt_index,
  source_agent_execution_id,
  source_work_item_id,
  backend_profile_id
}
PersistedHealthFallback { decision_id, backend_profile_id }
```

Static, dynamic, system-lead, and escalation variants resolve only frozen plan
or existing immutable ledger rows. A run-local provider-health fallback that
selects Codex first writes `ProviderHealthFallbackDecisionV1` with run,
occurrence, source execution, reason, source/target profile IDs, frozen catalog
hash, and decision digest in the same transaction that enqueues exact work.
The target pair is re-resolved from the frozen catalog. Payload fallback JSON
is comparison evidence only. Missing decision authority rejects before claim;
this does not introduce the deferred P079 provider-fallback mechanism.

#### Immutable occurrence and binding ownership

Migration-owned `exact_task_occurrences_v1` is the sole relational owner of a
canonical `TaskOccurrenceKeyV1`. It stores `id`, `run_id`, `kind`, canonical
JSON, SHA-256 digest, and exactly one source branch:

- static: `stage_execution_id` plus `frozen_task_index`;
- dynamic: `dynamic_materialization_record_id`, `materialization_epoch`,
  `selection_plan_hash`, `frozen_binding_id`, and `selection_index`; or
- P017: `mediation_record_id`.

Branch CHECK constraints, source FKs, `UNIQUE(run_id, digest)`, and immutable
UPDATE/DELETE triggers prevent cross-kind rebinding. Static rows are created
only after the StageExecution exists; the separate planned key remains the
pre-materialization identity. A dynamic row must reference the existing P060
materialization row whose run/stage/plan/binding values match. A P017 row must
reference a mediation in the same Run.

Migration-owned `exact_execution_binding_authorities_v1` stores `id`,
`run_id`, `occurrence_id`, `kind`, `backend_profile_id`, canonical JSON,
digest, frozen catalog hash, and branch FKs. Static/system-lead authority is
bound to the owning Run's immutable catalog hash; dynamic authority also
references its P060 materialization row and frozen binding ID. P058 authority
stores non-null `escalation_ledger_id`, `tier_id`, `tier_attempt_index`,
`source_agent_execution_id`, and `source_work_item_id`; all five must resolve
one existing escalation attempt in the same Run. Health authority references
a non-null `provider_health_fallback_decision_id`.
`UNIQUE(occurrence_id, digest)` plus immutable triggers make replay identity
stable while allowing a later, separately authorized tier/fallback attempt.

`provider_health_fallback_decisions_v1` owns the previously described decision
fields and has FKs to Run, occurrence, and source execution/work item. The
source/target profile IDs and catalog hash are stored scalars validated against
the frozen catalog because profiles are not relational rows. Its decision
digest and idempotency key are unique within the Run; its target pair is copied
only after re-resolution from that catalog.

The migration adds `task_occurrence_id`, `binding_authority_id`,
`backend_profile_id`, and `resolved_provider` to `work_items`; exact rows
require them and FK the occurrence/authority pair. Work also stores mutable
`current_exact_attempt_number`, which only the dedicated claim or pre-arm
recovery transaction may advance. `agent_executions` gains the same immutable
occurrence/authority/profile/provider values, positive `exact_attempt_number`,
and non-null `source_work_item_id`. Uniqueness on AgentExecution
`(task_occurrence_id, exact_attempt_number)` prevents duplicate attempt
identity while one work item may retain historical pre-prompt attempts.
`exact_invocation_attempts` FKs those rows and the durable generation.
Canonical JSON is retained for typed readback, but only indexed FK columns
authorize claim, replay, or settlement.

Five transactional APIs are the only writers:
`enqueue_exact_static_task_v1`, `enqueue_exact_dynamic_task_v1`,
`enqueue_exact_p017_mediation_v1`, `enqueue_exact_p058_tier_v1`, and
`enqueue_exact_health_fallback_v1`. Each validates the source row and frozen
catalog, creates or verifies immutable occurrence/authority rows, allocates the
next occurrence attempt number, and inserts the linked pending work item in
one transaction. Replaying the same source idempotency key returns the same
work item and attempt. A changed occurrence, source execution, P058 tier
attempt, fallback decision, profile, or catalog hash is a typed conflict and
inserts nothing.

`ExecutionRequest` also carries typed `SessionLaunchIntentV1`:

```text
LegacyUnspecified
Fresh {
  lineage_generation_id,
  occurrence_key,
  binding_authority
}
Reuse { session_generation_id, provider_session_id }
```

Existing serialized requests without this field decode as
`LegacyUnspecified`, which is legal only with `LegacyBestEffortV0`; exact mode
requires an explicit fresh intent.

The fresh lineage ID is durable audit/output ownership and is not a live
transport handle. Every exact invocation allocates and persists its canonical
generation before claim, even when `outputs` is missing or empty.
`ExactVariantV1` accepts only `Fresh` with one of the three occurrence variants
and a matching binding authority, with `reuse_existing_session == false` and
`keep_session_alive == false`; the transport always launches a process and
executes `session/new`. It rejects `Reuse`, P079 repair paths, P086
attach/resurrection, Steward ownership, and every other non-stage combination
before child launch. Generation allocation is therefore independent of output
materialization.

`StaticStageTask` and `DynamicStageTask` resolve exact bindings from their
frozen authorities. `LeadConflictMediation` resolves the existing P017 record, its
run, and that run's frozen `system_lead` binding; provider/model/effort in the
current mediation payload are comparison evidence only. Missing or mismatched
mediation identity rejects before launch. This preserves the existing P017
route without authorizing other non-stage owners. A non-Codex system lead and
a compiler-v1 mediation retain their existing provider/legacy path; the typed
exact intent is required only for a compiler-v2 Codex system lead.

Migration adds immutable Run columns `codex_exact_policy_id` and
`codex_exact_fixture_sha256`, populated only for a validated compiler-v2
snapshot, plus work-item column `execution_contract` with values
`legacy_v0` and `codex_exact_variant_v1`, defaulting old rows to `legacy_v0`,
plus nonnegative `exact_transition_seq` default zero. Existing
`pending/running/completed/failed/cancelled` status vocabulary remains.

DB triggers reject changing `execution_contract`; for an exact row every
status transition must atomically increment `exact_transition_seq` by exactly
one, and the sequence cannot change without a status transition. Dedicated
`transition_exact_work_item_v1` is the only exact status writer. A pre-change
claim/recovery/cancellation statement does not update the sequence, so SQLite
aborts it before mutation or child launch. Read-only capacity, fairness,
summary, storage-health, and run-readback queries continue to observe the
existing statuses without a second vocabulary.

New claim, recovery, cancellation, and settlement paths branch on immutable
provenance and use the dedicated CAS. A checked producer/status-writer
inventory classifies every SQL mutation site as exact-aware or provably
inapplicable. Downgrade may expose an old-daemon compatibility block or failed
claim transaction, but it cannot change an exact row or write prompt bytes.

DB INSERT/UPDATE guards also close the old-producer path. Every InvokeAgent row
for a Run with non-null v2 policy columns requires indexed provider/profile
values validated against the frozen catalog. Codex additionally requires
matching occurrence/authority rows and the exact contract; non-Codex requires
the legacy contract. A default row, payload-only route, or profile mutation
aborts. Therefore an old `AdvanceRun`, lazy materializer, P017, P058, or
health-fallback producer cannot insert or reroute work in a fresh-v2 Run; it
hits an explicit compatibility block. Compiler-v1 work keeps existing insertion
behavior, and a new daemon can still enqueue validated non-Codex work in a v2
Run.

An existing retry or P058 escalation caused by some unrelated failure may
create a new exact attempt only through the typed occurrence/binding authority
path and a new linked exact `pending` work item. A Codex tier is never
activated by mutating or reusing a legacy work item after claim; if the current
ledger path cannot materialize that exact item before child launch, it fails
closed. Cloning a legacy payload/status is forbidden. The three exact failure kinds
introduced by this slice never take that path.

Recovery may return exact `running` to exact `pending` only before a
durable prompt arm and only after atomically terminalizing the abandoned
AgentExecution, generation, and source claim. The next claim creates a new
attempt. Armed or dispatched exact work can never requeue and converges to the
terminal unknown settlement defined below.

#### Normative typed grammar

Rust serde uses internally tagged JSON with required
`{"schema_version":1,"kind":"..."}` prefixes:

| Type | `kind` branches and required fields |
|---|---|
| `CodexConfigurationModeV1` | `legacy_best_effort_v0`; `exact_variant_v1 { policy_id, fixture_sha256, backend_profile_id }` |
| `PlannedProviderIdentityModeV1` | `not_applicable`; `unavailable`; the two configuration-mode branches above |
| `TaskOccurrenceKeyV1` | `static_stage_task { stage_execution_id, frozen_task_index }`; `dynamic_stage_task { stage_execution_id, materialization_epoch, selection_plan_hash, frozen_binding_id, selection_index }`; `lead_conflict_mediation { mediation_record_id }` |
| `ExecutionBindingAuthorityV1` | the five branches and fields listed above |
| `SessionLaunchIntentV1` | `legacy_unspecified`; `fresh { lineage_generation_id, occurrence_key, binding_authority }`; `reuse { session_generation_id, provider_session_id }` |

Unknown versions/kinds/fields, duplicate keys, missing/null required fields,
wrong scalar types, and invalid cross-branch fields reject. Exact mode rejects
missing intent; only compiler-v1 legacy mode may default a missing field to
`legacy_unspecified`. Each object is at most 4 KiB UTF-8; IDs are 1...256 bytes
without control characters; SHA-256 values are exactly 64 lowercase hex bytes;
indices are nonnegative signed 64-bit values and attempt numbers are positive.

DB stores canonical JSON plus indexed occurrence kind, owner ID, task index,
and attempt number. Fixture directory
`control-plane/crates/domain/tests/fixtures/codex_exact_variant_v1/` contains
normative one-line bytes for every branch and malformed/unknown/cross-version
negatives. Rust authoritative types and Swift readback DTOs consume the same
configuration/occurrence fixtures. Swift treats an unknown V2 enum as
incompatible and clears rows; it never invents a legacy value.

A stage-scoped frozen fallback binding selected because of some other failure
may use exact mode only when its own frozen profile names an approved pair.
Configuration rejection itself is terminal and never schedules a retry,
fallback, escalation tier, repair, or resurrection. The caller cannot downgrade
an exact v2 request to legacy mode. V1 snapshots retain current legacy behavior.

### 3. Exact ACP configuration

For an exact-pair invocation one invocation-local
`CodexExactConfigurationHandshakeV1` actor owns the reader, option state, and
prompt writer. Its closed state machine is:

```text
awaiting_session_new -> setting_model -> setting_effort -> ready_to_prompt
any validation or transport failure --------------------------> rejected
ready_to_prompt -> prompt_write_started -> prompt_dispatched
```

It performs this sequence before writing any `session/prompt` bytes:

1. Launch a fresh Codex ACP process and send `session/new` with the exact model
   ID from the frozen binding.
2. Read the returned `configOptions` as the first working snapshot.
3. Resolve the unique `model` select option by one byte-exact ASCII full-value
   match. Alias,
   substring, token, display-name guessing, and fallback to an unadvertised raw
   value are forbidden.
4. Send required `session/set_config_option` with exact
   `{ sessionId, configId: "model", value: exact_model }` params.
5. Require the matching JSON-RPC response to contain a complete
   `configOptions` array whose model `currentValue` equals the exact requested
   model. That array replaces, rather than merges into, the working snapshot.
6. Resolve `reasoning_effort` from the updated working snapshot using the same
   exact-value rule.
7. Send required `session/set_config_option` with exact
   `{ sessionId, configId: "reasoning_effort", value: exact_effort }` params.
8. Require the matching response to contain a complete `configOptions` array
   proving both final
   `model.currentValue` and `reasoning_effort.currentValue` equal the requested
   pair.
9. Apply every exact ACP v1 configuration notification observed by the actor in
   wire order. The only accepted envelope is
   `session/update.params = { sessionId, update: { sessionUpdate:
   "config_option_update", configOptions } }`; `sessionId` must match and the
   complete array again replaces the working snapshot.
10. Reverify the current in-memory pair immediately before prompt delivery.
    The same actor, with no concurrent writer, requests an engine-owned durable
    prompt-write fence and waits for its commit acknowledgement before gaining
    access to the writer. It then sends the prompt.

The handshake accepts only responses with the exact request ID and only
`ConfigOption` entries with bounded ACP v1 shapes. Every complete replacement
must contain exactly one `model` option of type `select`, with its
`currentValue` present in its advertised values. During
`awaiting_session_new` and `setting_model`, `reasoning_effort` may be absent
because it can depend on the selected model. The complete response to the model
set and every later replacement must contain exactly one
`reasoning_effort` select option with the same current-value invariant. Unknown
options are allowed within the same bounds. A successful empty `{}` response
is intentionally rejected; compatibility with the old fake-ACP response is
not success evidence.

These request, complete-response, and complete-replacement notification shapes
follow the official
[ACP v1 session configuration contract](https://agentclientprotocol.com/protocol/v1/session-config-options).
Non-configuration session notifications retain normal transport handling and
cannot modify configuration proof.

Handshake-specific bounds are lower than the general transport budget:

- each configuration response or update NDJSON line is at most 256 KiB and
  parsed JSON depth is at most 16;
- at most 32 options and 64 advertised values per select option;
- option ID/category is at most 64 UTF-8 bytes, value/current value at most 128,
  option/value display name at most 256, and description at most 1,024; and
- at most 16 `config_option_update` notifications before the prompt.

Every response and notification is consumed in wire order. A success response
without the required bounded complete state is not proof. An observed provider
rejection, missing/duplicate/ambiguous option, wrong-session or malformed
response, limit overflow, or final mismatch returns
`ACP_CODEX_EXACT_CONFIGURATION_REJECTED`. A request send/read failure, timeout,
child exit, or host interruption before exact proof returns
`ACP_CODEX_EXACT_CONFIGURATION_UNPROVEN`. Both close the child and prove zero
prompt-write starts; only the former claims an observed incompatible provider
response.

The adapter reports a bounded reason through the engine callback. For an
observed rejection the callback must first commit
`configuration_rejection_observed`, then waits for terminal settlement before
returning the work result. A crash after that commit replays rejection. A crash
without that receipt remains neutral configuration-unproven; startup never
invents a provider rejection from process absence.

The ACP crate exposes a narrow callback interface but has no DB dependency.
Migration-owned table `exact_invocation_attempts` is keyed by
`(source_work_item_id, agent_execution_id)`, with unique generation ID and FKs
to the immutable occurrence/authority rows. One source work item may therefore
have multiple pre-prompt attempts, but each attempt has one idempotent row:

```text
claimed -> superseded | cancelled
claimed -> configuration_started -> child_started -> configuration_proved
configuration_started | child_started | configuration_proved -> cancelled
configuration_started | child_started | configuration_proved
  -> configuration_unproven
child_started -> configuration_rejection_observed -> configuration_rejected
configuration_proved -> prompt_write_started -> prompt_dispatched
prompt_write_started | prompt_dispatched -> prompt_delivery_unknown
prompt_dispatched -> terminal_result_recorded -> output_settled
```

Claim inserts `claimed` before any child launch. The engine commits
`configuration_started` before launching the ACP child; failure to commit
launches nothing. It commits `child_started` after spawn and
`configuration_proved` only after the final exact option snapshot is verified.
Recovery may supersede and requeue only `claimed`. Recovery from any other
nonterminal pre-arm state settles configuration-unproven unless an
observed-rejection row already owns the attempt. Recovery from
`prompt_write_started` or
`prompt_dispatched` settles prompt-delivery-unknown. This is a narrow
duplicate-prevention rule for exact invocations, not the deferred general
post-dispatch recovery contract.

Cancellation may CAS `claimed`, `configuration_started`, `child_started`, or
`configuration_proved` to `cancelled`, using the full Run/owner/work/execution/
generation/claim predicates and boundedly reaping any child. The observed
rejection CAS requires no cancellation request. Whichever CAS commits first
owns settlement: cancellation-first never becomes rejection; a committed
`configuration_rejection_observed` settles rejection and cannot be relabeled by
a later cancellation. After `prompt_write_started`, cancellation conservatively
settles prompt-delivery-unknown while preserving the existing canceled
owner/Run outcome. Retry supersession is legal only from `claimed`.

`exact_terminal_result_receipts_v1` has one immutable row per attempt with a
closed provider outcome, bounded redacted execution-result envelope, parsed
output-contract data or failure classification, artifact/transcript references,
and SHA-256 digest. It contains no unbounded raw provider payload. Receipt
insert and the CAS to `terminal_result_recorded` are one transaction. A crash
before commit leaves `prompt_dispatched` and converges to delivery-unknown; a
crash after commit replays only the receipt and never sends a second prompt.

`settle_exact_terminal_result_v1` deterministically feeds that receipt into the
existing output-contract settlement. Its transaction activates or rejects
outputs, closes work/execution/generation/claim and Stage or P017 ownership,
updates the attempt to `output_settled`, and records the existing terminal
event. Replay returns the same receipt and settlement. Success, provider
failure, output validation failure, and restart after receipt commit therefore
cannot strand running ownership or requeue provider work.

The engine arm transaction uses lock order Run, owner, work item,
AgentExecution, generation, source claim, exact attempt. It requires all of:

- the Run is nonterminal, not cancelling, and has no cancellation request;
- work status is `running`, its immutable contract is exact, its transition
  sequence matches the claimed value, and its current attempt matches the
  AgentExecution;
- AgentExecution is running and its owner, occurrence, attempt, and generation
  match the work item;
- generation and source claim are current/active with no supersession; and
- either the StageExecution is running with no settlement, or the P017
  mediation is running with its conflict in `lead_mediation_pending`.

One CAS changes only that attempt from `configuration_proved` to
`prompt_write_started` and returns an opaque one-use fence token. The writer
cannot be called without it. Predicate failure returns no token, closes/reaps
the child, and leaves terminal ownership to the transaction that won the race.
Failure to commit the fence likewise writes zero prompt bytes.

Once durable `prompt_write_started` is set, no later error is a configuration
rejection and no zero-prompt claim is allowed. A complete write CASes the same
attempt to `prompt_dispatched`; failure to commit or a later crash without a
terminal provider result remains delivery-unknown. Any short/partial write,
EPIPE, close, timeout, or other error from the first write attempt returns
`ACP_PROMPT_DELIVERY_UNKNOWN`, failure kind `prompt_delivery_unknown`, and
output settlement `none`; it closes the physical session and is ineligible for
automatic retry, repair, resurrection, fallback, or escalation. This narrow
settlement prevents duplicate work without claiming durable provider
acceptance.

Engine-owned `settle_exact_invocation_v1` is the sole terminal writer for
`configuration_rejected`, `configuration_unproven`, and
`prompt_delivery_unknown`. One immediate,
idempotent transaction CASes the attempt from its allowed predecessor, changes
the work item to `failed` through the fenced transition API, closes
AgentExecution with typed runtime
facts, invalidates generation/live ownership, and closes the active source
claim without activating output.

For a stage owner, that transaction sets StageExecution status `failed`,
settlement kind `failed`, and `completed_at`, then sets the parent Run to
`blocked` without changing `current_state`. For P017 it sets mediation status
`terminal_unverifiable`, its typed settlement result/recovery action and
`settled_at`; sets the workflow conflict to `terminal_unverifiable`; and keeps
the parent Run `blocked` at the same state. It enqueues no AdvanceRun, repair,
retry, fallback, or escalation work. Replays return the same attempt row.

Cancellation and retry-supersession use the same lock order and precedence
above. If a pre-arm cancellation commits first, arm returns no token and writer
count is zero. If arm commits first, cancellation records
`prompt_delivery_unknown`; retry supersession is refused and no replacement is
enqueued. Late provider output is quarantined and cannot reopen an exact
attempt or owner.

The in-memory option snapshot is invocation-local and discarded with the child;
it is not a durable acceptance receipt. Updates received after prompt dispatch
cannot create an accepted/configured UI claim because this slice exposes only
planned truth. The required path does not log option values, session
identifiers, or raw provider payloads. Existing bounded error/redaction
behavior remains in force.

### 4. Session lifetime

An exact-pair stage or P017 mediation invocation owns one fresh physical Codex
session. It is not eligible for cross-invocation live-session reuse or P086
resurrection in this slice. This slice does not authorize automatic retry of
configuration or prompt-delivery failures.

This deliberate performance tradeoff avoids treating process-local or
historical evidence as durable acceptance. Efficient reuse is deferred to the
provider-acceptance child document and cannot be enabled by a flag.

Legacy v1 frozen invocations retain their current reuse behavior.

### 5. Readback

One server-side `PlannedProviderIdentityClassifierV1` derives
`PlannedProviderIdentityModeV1` from an already compiled frozen plan for both
readback paths. Its closed mode vocabulary and precedence are:

1. non-Codex provider -> `not_applicable`;
2. missing execution-to-profile binding in an otherwise valid compiled plan ->
   `unavailable`;
3. valid compiler v2 marker, registry-resolved historical fixture, matching
   digest, and supported exact pair
   -> `exact_variant_v1`;
4. compiler v1 or absent marker -> `legacy_best_effort_v0`, even when its raw
   model string happens to name an exact variant.

Malformed, unknown-policy, digest-mismatched, generic, or otherwise invalid v2
snapshots fail in the existing authoritative compiler before this classifier
runs. Execution is blocked, no exact/legacy mode is emitted, and topology keeps
its current compile-failure behavior rather than fabricating a partial row.
Bounded non-authorizing inspection of corrupt snapshots belongs to the deferred
P031 readback proposal and is not required to test this fresh-valid hypothesis.

Existing `runStageTopology` and its types remain unchanged. Its resolver does
change internally: exact static executions are matched by persisted occurrence
key, never `agent_id`; exact dynamic/P017 rows that the old shape cannot
represent are excluded rather than cross-bound. Legacy rows retain their
existing best-effort matching.

Dedicated `runStageTopologyV2` is the Stages source after capability admission.
Every frozen static task has non-null `plannedTaskKey` based on
`{ stage_id, frozen_task_index }` even before lazy StageExecution allocation;
`taskOccurrenceKey` is nullable until materialization. Materialized dynamic
rows are appended from validated P060 occurrence records. P017 rows are
owner-aware and attach to `origin_stage_id` without inventing a
StageExecution. Occurrences carry planned model/effort and `configurationMode`
from the shared classifier plus nullable strings `failureKind`, `failurePhase`,
and `operatorActionHint`. The resolver joins an execution by persisted
occurrence key and selects attempts by the deterministic ordering above.
Dedicated `P031RunStageTopologyOccurrenceV2ReadModel` has the same fields and
does not infer mode or task identity locally. The legacy DTO remains unchanged.
Failure kind uses the retained raw
runtime-fact value, phase is a pure mapping for the three exact transport codes,
and action uses the existing runtime-fact value.

For exact rows, V2 status never trusts `stage_summaries` over canonical truth.
The resolver builds the frozen topology skeleton as today, then overlays work,
attempt, AgentExecution, StageExecution, P017 mediation/conflict, and Run rows
from one SQLite read transaction. Exact terminal settlement updates those
canonical rows atomically before returning. A contradictory or incomplete
exact join fails the V2 query instead of displaying a stale running row.
Projection rebuild may catch up independently; immediate and post-restart V2
readback therefore return the same terminal Stage/P017/Run state.

Existing `activeAgentExecutions: [GqlAgentExecution!]!` and its resolver remain
unchanged for old applications, fragments, generated clients, and rollback.
New field `activeAgentExecutionsV2` returns dedicated
`GqlActiveAgentExecutionV2` with nullable planned `effort`, non-null
`configurationMode`, and non-null `taskOccurrenceKey` for exact rows. It
otherwise preserves the old active field set and running-only semantics. Model
and effort are derived together from the execution's `backend_profile_id` and
persisted occurrence key in the run's frozen catalog. It does not read the
current catalog. Every producer of this dedicated type uses the same helper.
P017 active rows resolve through mediation owner ID and frozen system lead;
they do not require an inner join to StageExecution.
Terminal failure presentation belongs to `runStageTopologyV2`; a terminal row
is intentionally absent from running-only `activeAgentExecutionsV2`.
Overview consumes only `activeAgentExecutionsV2` and IDs rows by
AgentExecution ID; it never derives rows from topology occurrence stacks.
Stages consumes only `runStageTopologyV2` and IDs each occurrence by the
canonical occurrence digest, falling back to the planned-task digest before
static materialization. Multiple attempts remain one Stage occurrence with
the deterministic latest attempt and `executionCount`; repeated agents in
different static, dynamic, or P017 occurrences cannot collapse together.

GraphQL exposes:

```graphql
enum PlannedProviderIdentityKindV1 {
  NOT_APPLICABLE
  UNAVAILABLE
  LEGACY_BEST_EFFORT_V0
  EXACT_VARIANT_V1
}

type PlannedProviderIdentityV1 {
  schemaVersion: Int!
  kind: PlannedProviderIdentityKindV1!
  policyId: String
  fixtureSha256: String
  backendProfileId: ID
}

enum TaskOccurrenceKindV1 {
  STATIC_STAGE_TASK
  DYNAMIC_STAGE_TASK
  LEAD_CONFLICT_MEDIATION
}

type TaskOccurrenceKeyV1 {
  schemaVersion: Int!
  kind: TaskOccurrenceKindV1!
  stageExecutionId: ID
  frozenTaskIndex: Int
  materializationEpoch: String
  selectionPlanHash: String
  frozenBindingId: ID
  selectionIndex: Int
  mediationRecordId: ID
}

type PlannedTaskKeyV1 {
  stageId: ID!
  frozenTaskIndex: Int!
}

enum ModelVariantExecutionOwnerKindV1 {
  PLANNED_STATIC_TASK
  STAGE_EXECUTION
  LEAD_CONFLICT_MEDIATION
}

type GqlActiveAgentExecutionV2 {
  id: ID!
  ownerKind: ModelVariantExecutionOwnerKindV1!
  ownerId: ID!
  stageExecutionId: ID
  mediationRecordId: ID
  originStageId: ID!
  taskOccurrenceKey: TaskOccurrenceKeyV1
  attemptNumber: Int
  agentId: String!
  agentTitle: String
  provider: String!
  model: String
  effort: String
  configurationMode: PlannedProviderIdentityV1!
  status: String!
  startedAt: String!
  completedAt: String
  stageLabel: String
  taskLabel: String
  lastEventAt: String
  eventCount: Int
  selectionOrder: Int
  selectionUnavailableReason: String
  sessionLineageId: ID
  sessionGenerationId: ID
}

type GqlRunStageTopologyOccurrenceV2 {
  id: ID!
  ownerKind: ModelVariantExecutionOwnerKindV1!
  ownerId: ID
  stageExecutionId: ID
  mediationRecordId: ID
  originStageId: ID!
  plannedTaskKey: PlannedTaskKeyV1
  taskOccurrenceKey: TaskOccurrenceKeyV1
  agentExecutionId: ID
  attemptNumber: Int
  agentId: String!
  agentTitle: String!
  taskName: String!
  status: String!
  provider: String!
  model: String
  effort: String
  configurationMode: PlannedProviderIdentityV1!
  executionCount: Int!
  failureKind: String
  failurePhase: String
  operatorActionHint: String
}

type GqlRunStageTopologyTransitionV2 {
  toStageId: ID!
  toLabel: String
  detail: String
}

type GqlRunStageTopologyNodeV2 {
  stageId: ID!
  label: String!
  order: Int!
  ownerAgentId: String!
  ownerAgentTitle: String!
  status: String!
  isCurrent: Boolean!
  iteration: Int
  attemptNumber: Int
  startedAt: String
  completedAt: String
  approvalRequired: Boolean!
  artifactCount: Int!
  communicationCount: Int!
  occurrences: [GqlRunStageTopologyOccurrenceV2!]!
  transitions: [GqlRunStageTopologyTransitionV2!]!
}

input DaemonGenerationInputV1 {
  endpoint: String!
  pid: String!
  startedAtUnixNanos: String!
  buildSha: String!
}

type DaemonGenerationV1 {
  endpoint: String!
  pid: String!
  startedAtUnixNanos: String!
  buildSha: String!
}

type CodexModelVariantCapabilityV1 {
  compatible: Boolean!
  generation: DaemonGenerationV1!
  generationToken: String!
}

codexModelVariantReadbackV1(
  expectedGeneration: DaemonGenerationInputV1!
): CodexModelVariantCapabilityV1!
activeAgentExecutionsV2(runId: ID!, generationToken: String!):
  [GqlActiveAgentExecutionV2!]!
runStageTopologyV2(runId: ID!, generationToken: String!):
  [GqlRunStageTopologyNodeV2!]!
```

For `PlannedProviderIdentityV1`, only `EXACT_VARIANT_V1` has non-null policy,
fixture, and profile fields; every other kind requires them null. A
`TaskOccurrenceKeyV1` has exactly the fields of its Rust branch. Active V2 rows
always have a materialized owner. Exact rows additionally require non-null
occurrence and attempt; legacy rows retain nullable occurrence/attempt fields.
P017 has null `stageExecutionId`, non-null `mediationRecordId`, and its real
`originStageId`. Stage topology planned-static rows use
`PLANNED_STATIC_TASK`, null owner/execution/occurrence/attempt, and non-null
`plannedTaskKey`. Materialized stage rows have a StageExecution owner; P017
has a mediation owner and no fabricated StageExecution. Dynamic and P017 rows
have null `plannedTaskKey`. Legacy occurrences may have null occurrence and
attempt but carry the closed legacy/unavailable/not-applicable mode. Schema
snapshots and generated Swift fixtures enforce this nullability table.

The existing daemon status payload adds a server-derived
`daemonGenerationV1` object with the same four fields. An old daemon or a
failed-serve response without it is incompatible and cannot authorize V2
queries. `endpoint` is the daemon's configured advertised GraphQL URL,
canonicalized once at startup: absolute ASCII `http`/`https`, lowercase scheme
and DNS host, normalized IP literal, explicit decimal port, no userinfo/query/
fragment or dot segments, and path exactly `/graphql`. Aliases such as
`localhost` and `127.0.0.1` are not interchangeable after startup.

`pid` is 1...20 ASCII decimal digits with no leading zero;
`startedAtUnixNanos` is the process start instant as 1...19 positive decimal
Unix nanoseconds with no leading zero. Swift retains that string and never
round-trips it through `Date`. `buildSha` is empty for a development build or
40/64 lowercase hex bytes. Incomplete or noncanonical status identity fails
closed before probe. The probe compares each expected field byte-for-byte with
its serving process and returns `DAEMON_GENERATION_CHANGED` on mismatch.

`generationToken` is lowercase 64-hex SHA-256 over bytes
`codex-model-variant-generation-v1\0`, then `endpoint`, `pid`,
`startedAtUnixNanos`, and `buildSha` in that order, each as a four-byte
big-endian byte length followed by canonical UTF-8 bytes. The server derives
the token from its own startup tuple after comparison, never from echoed
caller bytes; Swift independently verifies it from the unmodified status
strings. It is not an authorization token. Both V2 resolvers compare it with
their current tuple before loading run data and return
`DAEMON_GENERATION_CHANGED` on mismatch. Shared Rust/Swift golden vectors cover
empty development SHA, IPv4, DNS, maximum PID/nanoseconds, and every rejected
noncanonical spelling.

An actor-owned
`ModelVariantCapabilityCoordinator` keys state by exact daemon generation
`{ endpoint, pid, started_at_unix_nanos, build_sha }` from current status
readback. Its closed states are `unknown`, `probing`, `compatible`,
`incompatible`, `failed`, and `generation_changed`; one single-flight probe
exists per generation. Only an error-free
response with `compatible == true` and a valid bounded generation token is
compatible only when returned generation equals the expected status tuple.
`false`, missing data, unknown-field response, partial data with errors,
timeout, generation mismatch, and decode failure never authorize a versioned document.
`false`, missing data, and unknown-field errors become `incompatible`; partial
errors, timeout, and decode failure become `failed`. The server returns true
only when both dedicated V2 resolvers and occurrence/failure fields are
installed. A generation-key change invalidates prior state and token.

Before selecting either V2 resolver, the app completes that probe. Compatible
state permits the versioned run-detail document with the returned token and
decodes dedicated `P031ActiveAgentExecutionV2ReadModel` with nullable
stage/occurrence/attempt where the SDL allows it. The legacy DTO remains
unchanged.
Incompatible or failed state shows a blocking message and does not send the
document.

Run-detail loading is keyed by `{ run_id, generationToken, request_nonce }`.
Changing the selected Run or daemon generation immediately clears prior V2
rows, cancels the old task, and enters `unknown`/`probing`; a late response is
discarded unless all three keys still match. `unknown`, `probing`, `failed`,
`incompatible`, and `generation_changed` render distinct safe placeholders and
never retain rows from the previously selected Run.

Recovery actions are closed and state-specific:

| State | Operator action | Behavior |
|---|---|---|
| `unknown` / `probing` | none | keep the safe placeholder and await the single-flight probe |
| `failed` | `Retry Readback` | retry the same tuple with a new nonce; never restart |
| `incompatible` | `Restart Daemon` | invoke only the existing explicit operator command after warning that active work can be interrupted |
| `generation_changed` | none | clear token/rows and boundedly await a distinct ready status generation before probing |

`DAEMON_GENERATION_CHANGED` enters the last state, polls status for at most 30
seconds, and probes only a distinct ready tuple. It never immediately probes
the stale tuple or restarts the daemon. Timeout becomes `failed` and offers
only `Retry Readback`. The explicit restart path uses the same bounded wait;
returning the same tuple is not success. Capability handling never restarts or
replaces the daemon automatically, transfers state across generations, or
interrupts active work on its own. Old documents continue to work against the
new daemon.

MCP, reports, artifacts, receipts, and runtime health keep their existing
shapes. The new failure kinds use their existing bounded string/raw-value lanes;
no new `OperatorActionHint` vocabulary is introduced.

### 6. Shared presentation

One pure formatter owns model/effort copy for Overview active-agent rows and
Stages occurrence rows. It receives only provider, planned model, planned
effort, and the server-derived configuration mode. Swift never reclassifies a
raw snapshot.

Presentation ownership is intentionally separate:
`ModelVariantOverviewRowModel` accepts only running
`P031ActiveAgentExecutionV2ReadModel` values and has no terminal-failure fields;
`ModelVariantStageOccurrenceRowModel` accepts topology occurrences and owns
failure kind/phase/recommendation. Neither V2 surface uses the shared legacy
`P036StageOccurrenceRow` as its presentation model. They share only the pure
identity formatter and copy handler.

Exact Codex examples:

```text
Codex · Sol (gpt-5.6-sol) · max · planned
Codex · Terra (gpt-5.6-terra) · high · planned
Codex · Luna (gpt-5.6-luna) · high · planned
```

Legacy example:

```text
Codex · gpt-5.6 · high · legacy planned/unverified
```

Classifier-unavailable example (normative full formatter output):

```text
Codex · planned configuration unavailable
```

Rules:

- `Sol`, `Terra`, and `Luna` are friendly labels only when mode is
  `exact_variant_v1`; the same raw ID in legacy mode remains raw legacy text.
- Each Stage and Overview row gives highest visual priority to line 1, agent
  name plus status, and line 2, Codex variant plus effort plus `planned`. At the
  current 292-point constrained width these values remain readable without
  truncation or overlap; wrapping is allowed at larger accessibility sizes.
- The raw exact ID may wrap or truncate visually after those values, but the
  row's help text exposes the complete formatter output.
- Each row has a trailing icon-only `Copy Model Configuration` button, its
  context-menu alias, and a named accessibility action backed by one copy
  handler. The button is in normal keyboard traversal, has visible focus,
  tooltip/help text, and copies the complete untruncated formatter output
  without changing row selection. There is no app-level Commands-menu item or
  window-global shortcut in this slice, so copy ownership cannot cross windows.
- `planned` is mandatory. This slice never renders `accepted`, `configured`,
  `actual`, or equivalent claims.
- Missing effort renders `effort unavailable`; unknown nonempty values render
  bounded escaped text and never map to a known effort.
- Mode `unavailable` always emits exactly the normative unavailable copy. It
  exposes no friendly variant, raw model, or effort in visible text, help,
  copied value, accessibility value, logs, or diagnostics assembled by this
  formatter.
- Non-Codex providers retain their existing requested-identity copy.
- Each rendered agent row owns one accessibility element. Its label contains
  agent and task, its value contains status plus the complete formatter output, and
  its hint describes only an existing action. Parent cards must not combine or
  hide occurrence accessibility children. Formatting must not change focus or
  selection.
- In Stages, `provider_configuration_rejected` renders `Configuration rejected`,
  `provider_configuration_unproven` renders `Configuration not proven`, and
  `prompt_delivery_unknown` renders `Prompt delivery unknown`. When the retained
  raw action hint is `inspect_logs`, the row shows noninteractive text `Inspect
  daemon logs`; this slice adds no button, navigation destination, or
  accessibility action for it. The complete failure kind, phase, and raw action
  hint remain in the row's accessibility value. No generic retry copy is shown
  for these terminal failures. Overview remains running-only and does not claim
  to present terminal failures.

Unknown values pass through one cross-language `BoundedIdentityScalarV1`:

- invalid UTF-8 or upstream input over 256 UTF-8 bytes becomes `unavailable`;
- trim only ASCII space, tab, CR, and LF at both ends; an empty result becomes
  `unavailable`;
- perform no Unicode normalization or case folding;
- escape backslash as `\\`; escape C0, C1, DEL, and all embedded line breaks as
  uppercase `\u{HEX}` with no raw control character in output; and
- cap escaped output at 96 UTF-8 bytes including ASCII marker
  `...[truncated]`, cutting only at a Unicode-scalar or complete-escape
  boundary.

Known model and effort vocabulary is matched byte-exactly before the unknown
formatter path. Rust and Swift consume the same golden scalar fixtures.

The Stages and Overview views must call the same formatter. A source scan and
view tests reject independent string assembly for these two surfaces.

## Failure behavior

| Failure | Result | Prompt delivery |
|---|---|---|
| Fresh matrix mismatch | Compile error; no run created | Write not started |
| Frozen binding/payload mismatch | Typed rejection before child launch | Write not started |
| Exact model option absent/ambiguous | `ACP_CODEX_EXACT_CONFIGURATION_REJECTED` | Write not started |
| Model set response lacks exact current value | Same typed failure | Write not started |
| Effort option absent/ambiguous | Same typed failure | Write not started |
| Final pair mismatch | Same typed failure | Write not started |
| Observed incompatible/rejected configuration | `ACP_CODEX_EXACT_CONFIGURATION_REJECTED` | Write not started |
| Configuration send/read/child failure before proof | `ACP_CODEX_EXACT_CONFIGURATION_UNPROVEN` and bounded reap | Write not started |
| Restart from pre-arm state without rejection receipt | Same neutral unproven settlement | Write not started |
| Restart after rejection receipt | Configuration-rejected settlement | Write not started |
| Invalid exact v2 provenance/request shape | Strict compile failure before child launch | Write not started |
| Prompt write fails after attempt begins | `ACP_PROMPT_DELIVERY_UNKNOWN` | Unknown; never reported as zero |
| Restart from armed/dispatched without terminal result | Same typed terminal settlement | Unknown; never requeued |
| Restart after terminal-result receipt | Replay receipt into existing output settlement | No second write or prompt |
| Legacy v1 frozen run | Existing legacy path; UI says unverified | Existing behavior |
| Valid plan cannot resolve execution profile | `effort = null`; UI says unavailable | No mutation |

`ACP_CODEX_EXACT_CONFIGURATION_REJECTED` maps to new domain failure kind
`provider_configuration_rejected`, `failure_kind_version = 2`, failure phase
`provider_configuration`, output settlement `none`, existing operator action
hint `inspect_logs`, and `retryable = false`. The failed AgentExecution and its
transport code are immutable; the adapter invariant and instrumented writer
prove that prompt write was not started. Cleanup terminates and reaps the child
within the existing bounded cleanup policy. The common exact terminal
transaction closes work/execution/generation/claim and the real stage or P017
owner states atomically; generic work-item failure/AdvanceRun writers are not
used.

`ACP_CODEX_EXACT_CONFIGURATION_UNPROVEN` maps to new domain failure kind
`provider_configuration_unproven`, `failure_kind_version = 2`, the same
`provider_configuration` phase, output settlement `none`, existing
`inspect_logs`, and `retryable = false`. It asserts only that exact
configuration was not durably proved before the child disappeared; it never
claims the provider rejected the pair. Cleanup and the common terminal owner
settlement are identical to configuration-rejected.

`ACP_PROMPT_DELIVERY_UNKNOWN` maps to new failure kind
`prompt_delivery_unknown`, `failure_kind_version = 2`, failure phase
`prompt_delivery`, output settlement `none`, existing hint `inspect_logs`, and
`retryable = false`. Its distinct transport code proves that the durable write
fence was committed and never records a zero-prompt assertion. The execution
enters the existing fresh-session quarantine/late-output isolation path and
places its stage or mediation owner in terminal operator hold, so possible
side effects or late outputs cannot be consumed by later automatic work.

All three exact terminal failure kinds are ineligible for automatic retry, P058 escalation tiers,
P079 output repair, P086 resurrection, provider-health fallback, provider
switching, and weaker/default model selection. No retry ledger or new action
hint is introduced by this slice.

Persistence retains the raw new failure string in the existing bounded
`failure_kind_raw_debug` lane and maps it to `Unknown` for an old reader that
does not know version 2. The existing GraphQL `AgentFailureKind` enum remains
unchanged and likewise emits `UNKNOWN`; V2 readback carries the bounded raw
string separately. MCP and reports continue to expose their existing
nullable/string lanes. Because all three rows use existing `inspect_logs`, old
`OperatorActionHint` decoding remains valid. Compatibility tests cover
old-reader/new-row readback on DB, GraphQL, MCP, and report projections.

## Verification gate

Add focused gate `codex-model-variant-slice`. It is provider-free and runs:

1. Strict fixture parsing, duplicate-key negatives, the exact v1 literal
   digest/length, rejection of in-place v1 mutation/removal, parity across
   checked-in/Rust-embedded/Xcode-bundled bytes, and coexistence of v1 with a
   synthetic later policy. A dependency scan proves all Rust consumers use
   `domain::codex_model_variant_policy` and no second registry exists.
2. Rust fresh compiler positives for every approved production row and bounded
   non-production fixture catalogs using supported exact pairs.
3. Mutation negatives for every production profile, generic model, unknown
   model/effort, Luna `ultra`, duplicate profile, missing canonical production
   profile, and undeclared extra production Codex profile.
4. Separate fresh and frozen admission tests: fresh compilation writes v2;
   every v1 snapshot remains byte-identical legacy even with an exact-looking
   pair; malformed/unknown-policy v2 fails strict compilation and emits no
   partial planned-identity row.
5. Producer/authority tests cover static and P060 dynamic tasks, P017 frozen
   system lead, persisted P058 escalation tier, and persisted run-local health
   fallback through their five transactional APIs. Schema tests prove branch
   CHECKs, FKs, immutability, and source idempotency. Every pair is re-resolved
   from frozen authority; payload-only, mismatched plan/binding/decision,
   duplicate-agent, cross-run/cross-occurrence replay, changed source execution,
   changed P058 ledger/tier/tier-attempt, and valid-pair substitution negatives
   reject before launch. A producer inventory requires every InvokeAgent
   enqueue site to classify as exact-aware or fail-closed.
6. Occurrence tests cover static frozen indices, P060 materialization identity,
   P017 owner identity, immutable authority FKs, unique occurrence attempt
   allocation, deterministic attempt order, idempotent replay, and outputless
   tasks. Absent, empty, and nonempty outputs all allocate durable generations
   and execute `session/new`.
7. Shared byte-fixture tests cover every branch of
   `CodexConfigurationModeV1`, `TaskOccurrenceKeyV1`,
   `ExecutionBindingAuthorityV1`, and `SessionLaunchIntentV1`, plus duplicate,
   malformed, unknown-version/kind, missing/null, size, and cross-branch
   negatives. Missing intent defaults only for legacy mode.
8. Downgrade/lifecycle tests create exact-contract rows in every existing
   status with the new binary and run old claim/recovery/cancellation statements
   against the same DB. Triggered writes abort atomically, provenance/sequence
   remain unchanged, and child/prompt counts stay zero. Old `AdvanceRun`, lazy
   static/P060, P017, P058, and health-fallback inserts/reroutes against a
   fresh-v2 Run also abort with zero new work or launch. Dedicated transition
   tests plus the checked SQL inventory cover claim, pre-arm recovery and
   cancellation, while read tests cover capacity, fairness, queue summaries,
   storage health, and run readback.
9. Fake ACP success proving exact request IDs, full-state replacement,
   dependent effort appearing only after model selection, notification
   interleaving, ordered model then effort configuration, and exactly one
   prompt after both exact `currentValue` checks.
10. Fake ACP negatives for alias-only, substring-only, duplicate, missing,
   malformed, empty-success, stale-snapshot, wrong-session, out-of-order,
   rejected, mismatched, and every numeric-limit overflow; each asserts zero
   prompt-write starts and bounded child cleanup.
11. Attempt-state fault injection at every durable boundary proves: only
    pre-launch `claimed` may create a later attempt; pre-arm crash without a
    rejection receipt converges to neutral configuration-unproven; an observed
    rejection receipt converges to configuration-rejected; neither writes a
    prompt. Cancellation races at `claimed`, `configuration_started`,
    `child_started`, `configuration_proved`, and rejection observation prove the
    documented first-CAS precedence and full owner predicates. All terminal
    paths close work/execution/generation/claim and real Stage/P017/Run states
    atomically with no AdvanceRun/retry/fallback.
12. Arm race tests prove full ownership predicates and lock order against
    Run/Stage/P017 cancellation and retry supersession. Cancellation-first
    produces zero writes; arm-first produces one terminal unknown settlement.
    Short writes at every byte boundary and restart from armed/dispatched prove
    no requeue, no duplicate prompt, no repair/resurrection/fallback/escalation,
    and quarantine of late output. Terminal-result receipt fault injection for
    provider success/failure and valid/invalid output proves crash-before-receipt
    becomes delivery-unknown, while crash-after-receipt deterministically
    settles once with no second prompt or stranded Stage/P017 ownership.
13. Old-reader/new-row compatibility across DB, GraphQL, MCP, and reports,
    proving all three raw version-2 failure values are retained, old GraphQL enum value `UNKNOWN`,
    and existing `inspect_logs` hint.
14. GraphQL compatibility tests prove old active/topology fields and documents
    work unchanged on the new daemon and the new app never sends either V2
    document to an old daemon. The legacy topology resolver occurrence-matches
    exact static rows and excludes unrepresentable exact dynamic/P017 rows.
    An SDL snapshot and generated Swift fixtures assert every V2 field,
    nullability branch, nested mode/key type, and owner-aware P017 row.
15. Capability tests cover exact expected/returned generation equality,
    shared Rust/Swift canonical endpoint/time/token vectors, server derivation
    independent of caller bytes, every noncanonical spelling, incomplete
    identity, false/missing/partial, timeout/decode, concurrent callers, and the
    closed action matrix. `failed` only retries, `incompatible` only restarts,
    and `DAEMON_GENERATION_CHANGED` clears state and waits for a distinct ready
    generation before probing. Daemon A/B and Run A/B tests prove stale or
    same-generation responses cannot populate current rows.
16. GraphQL tests prove both V2 paths use the shared classifier and occurrence
    join, derive from frozen/persisted authority rather than current catalog or
    payload, represent future lazy static rows with planned-only identity,
    include dynamic and owner-aware P017 rows after materialization, preserve
    deterministic attempt order, and expose terminal failures only in Stages.
    Immediate, projection-lag, and post-restart reads prove canonical terminal
    Run/Stage/P017 coherence. Duplicate-agent fixtures prove Overview IDs by
    execution and Stage IDs by occurrence/planned key for static, dynamic, P017,
    and repeated attempts; Overview never consumes topology stacks.
17. Swift decoding, bounded-scalar, and formatter goldens for Sol, Terra, Luna,
    exact-looking legacy, generic legacy, missing effort, unknown bounded
    values, exact unavailable copy with no leaked model/effort, all three
    terminal failure presentations, compact copy, full copy, and accessibility
    output.
18. Hosted Overview and Stages tests at 292 points with `.large` and
    `.accessibility3` text proving friendly variant, effort, planned qualifier,
    and status remain distinguishable; the row-local button is keyboard and
    pointer operable, and context-menu/accessibility aliases return the same
    full value without moving selection. Separate surface-model tests prove
    terminal failure is absent from Overview and complete in Stages. Two-window
    tests prove no cross-window copy ownership, and parent accessibility does
    not hide rows.
19. Structural scans prove there is no feature flag, environment bypass,
    current-catalog read in run readback, automatic capability-triggered daemon
    restart, replacement of either old GraphQL field, app-level copy command,
    interactive inspect-logs affordance, shared V2 presentation row, or second
    formatter/registry.

The gate fails when any selected Rust or Swift suite executes zero tests. It
does not invoke a live provider, network, remote UI host, or another proposal
gate.

## Rollout

- The approved matrix and exact ACP sequence become default behavior for every
  newly compiled run after release.
- There is no disable switch, experiment percentage, or operator opt-in.
- Pre-change frozen runs continue unchanged and visibly say legacy/unverified.
- Exact work keeps immutable `codex_exact_variant_v1` provenance and a
  DB-enforced transition sequence across claim, recovery, cancellation, and
  settlement; an old daemon cannot claim, requeue, or launch it.
- Configuration rejection, configuration-unproven, and
  prompt-delivery-unknown settle terminally and visibly; retry, repair,
  resurrection, fallback, provider switch, and escalation do not react.
- A committed terminal-result receipt replays into existing output settlement;
  it never requeues or sends a second prompt after restart.
- A stale daemon produces a visible compatibility block. Capability probing
  never restarts it; restart remains an explicit operator action.
- Operational observation from a normal later run is useful but not required
  to merge this provider-free slice.

## Acceptance checklist

- [ ] The active catalog contains exactly the approved seven Codex pairs.
- [ ] Policy v1 is pinned to the normative digest/length, retained
      append-only, and can coexist with a later policy without invalidating old
      exact snapshots.
- [ ] The Rust compiler rejects every matrix mutation before run creation;
      Swift `YAMLValidator` reports the same mutations as non-authoritative
      preflight feedback.
- [ ] Fresh snapshots carry the validated v2 marker; every v1 snapshot,
      including exact-looking values, remains on typed legacy behavior.
- [ ] Existing generic frozen runs replay without byte mutation and without a
      fabricated Sol/Terra/Luna label.
- [ ] Exact mode derives and rechecks its pair from frozen
      `backend_profile_id`; another valid pair cannot be substituted.
- [ ] Static, P060 dynamic, and P017 exact occurrences have persisted typed
      keys, immutable relational owners, and deterministic attempt order; P058
      authority binds ledger, tier, tier-attempt, and source identities, while
      run-local health fallback binds its persisted decision. Transactional
      producer replay is idempotent; payload-only, cross-run, cross-binding, or
      repeated-agent identity cannot authorize execution.
- [ ] Exact mode allocates fresh durable lineage with absent, empty, or nonempty
      outputs; typed P017 mediation resolves the frozen system-lead binding,
      while physical reuse, keep-alive, repair, resurrection, supplied live
      sessions, Steward, and every other non-stage owner reject before launch.
- [ ] Normative typed JSON/DB/Swift fixtures cover every exact mode,
      occurrence, binding-authority, and launch-intent branch and reject every
      malformed/unknown/cross-version shape.
- [ ] Exact-v1 work retains immutable provenance and DB-fenced transitions
      through claim/recovery/cancellation/settlement; pre-change claim and
      recovery cannot mutate or launch it, and old producer inserts/reroutes
      against a fresh-v2 Run abort before work creation. Prompt write count
      remains at most one.
- [ ] Exact invocations verify model and effort in order before the first
      prompt, and every pre-prompt negative fixture proves zero prompt-write
      starts.
- [ ] Exact invocations use a fresh physical session and none of the three exact
      terminal failures enters automatic retry or escalation.
- [ ] Attempt-state fault injection proves pre-arm cancellation precedence,
      observed rejection only from its durable receipt, and neutral
      configuration-unproven recovery otherwise. Each atomically closes exact
      work, execution, generation, claim, and real Stage/P017/Run ownership
      with zero prompt writes and no generic AdvanceRun/retry.
- [ ] Full ownership-CAS race tests prove no writer access before the durable
      fence; cancellation-first writes zero bytes, while arm-first, every
      partial write, and armed/dispatched restart converge to one idempotent
      unknown settlement without startup requeue.
- [ ] A terminal-result receipt is committed with attempt state and replayed
      atomically into existing output settlement for provider success/failure
      and valid/invalid outputs; restart cannot strand ownership or send a
      second prompt.
- [ ] Overview and Stages show the same friendly variant, effort, and `planned`
      qualifier and expose the complete exact model ID through the shared full
      value, help, and copy affordance.
- [ ] Active-agent effort is derived only from the frozen backend profile and
      remains nullable when unavailable.
- [ ] Old GraphQL shapes remain unchanged and exact rows cannot cross-bind in
      their legacy resolver; V2 exposes planned-only lazy identity plus
      materialized static/dynamic/P017 occurrence identity. Complete SDL,
      nullability snapshots, and generated-client fixtures represent P017
      without a fabricated StageExecution.
- [ ] V2 overlays canonical exact terminal truth during projection lag and
      after restart. Overview uses execution IDs and no topology stacks; Stages
      uses occurrence digest with planned-key fallback, so duplicate agents,
      tasks, and attempts do not collapse or leak terminal rows into Overview.
- [ ] Expected/returned generation equality plus the normative token binds the
      single-flight capability probe and both V2 documents to one daemon. The
      server-derived canonical endpoint/PID/Unix-nanos/build tuple has shared
      Rust/Swift vectors and is never hashed from caller-normalized bytes.
- [ ] Capability recovery has the closed action matrix: failed can only retry,
      incompatible can only explicitly restart, and generation-changed clears
      stale state then boundedly waits for a distinct ready generation before
      probing. It never restarts automatically.
- [ ] Typed configuration rejection cannot enter repair, resurrection,
      fallback, provider switching, or escalation.
- [ ] Old readers retain raw new failure values and decode the existing
      `inspect_logs` action; no new action-hint vocabulary is introduced.
- [ ] Separate Overview-running and Stages-occurrence presentation models keep
      terminal configuration states out of Overview and visible in Stages;
      complete model configuration is keyboard, pointer, and accessibility
      readable through row-local actions without moving selection or crossing
      windows.
- [ ] Classifier mode unavailable renders exactly `Codex · planned
      configuration unavailable` in visible/help/copy/accessibility output and
      exposes no friendly variant, raw model, or effort.
- [ ] `inspect_logs` is rendered only as noninteractive recommendation text;
      this slice does not promise a destination it does not implement.
- [ ] No public surface claims accepted/configured/actual provider identity.
- [ ] No flag or bypass can disable the fresh-run behavior.
- [ ] `./scripts/test-gate.sh codex-model-variant-slice` passes with nonzero
      Rust and Swift test counts.

## Decomposition

The following documents preserve the independent scope removed from the
checkpoint. They are deferred roadmap inputs and must receive separate design,
review, implementation, and closeout cycles before use:

| Deferred child | Removed responsibility | Inherited review findings |
|---|---|---|
| [Provider accepted truth and prompt authority](2026-08-31-provider-accepted-truth-and-prompt-authority-design.md) | Durable accepted configuration, general occurrence authority beyond this slice's static/P060/P017 keys, reuse, prompt permits, and general post-dispatch recovery/settlement beyond this slice's conservative no-requeue terminalization | P2-01 and accepted-truth portions of the checkpoint |
| [Provider configuration migration and reconciliation](2026-08-31-provider-configuration-migration-and-reconciliation-design.md) | Class A registry, append-only reconciliation, bootstrap migration phases and manifests | P1-01, P1-05 |
| [P079 repair output materialization](2026-08-31-p079-repair-output-materialization-design.md) | Staging, leases, chunk resume, history activation, crash recovery | P1-02 |
| [P086 resurrection containment](2026-08-31-p086-resurrection-containment-design.md) | Claude attach protocol, secret resolver, root/MCP containment, output-only recovery | P1-03 |
| [Provider egress and diagnostics containment](2026-08-31-provider-egress-and-diagnostics-containment-design.md) | Endpoint authority, DNS/TLS/redirect policy, direct-network denial, debug sink | P1-04 |
| [P031 bounded runtime readback](2026-08-31-p031-bounded-runtime-readback-design.md) | Complete operation inventory, paging, typed topology errors, bounded counters | P1-06, P1-07, P2-03 |
| [Frozen run replacement and input repair](2026-08-31-frozen-run-replacement-and-input-repair-design.md) | No-oracle API, ARCH-002 settlement, request-body cap, repair workspace | P1-08, P2-02 |
| [Verified provider truth UI](2026-08-31-verified-provider-truth-ui-design.md) | Accepted/configured states, Timeline integration, advanced focus and accessibility matrices | Advanced UI portions of the checkpoint |

Reviewer assesses this active document, including whether it accidentally
depends on deferred behavior. A boundary leak is a valid finding. Deferred
children are not required to be implementation-ready when this slice does not
depend on them.

## Scope-budget check

This document must remain below 2,000 physical lines. Any review request that
would take it to or beyond the limit creates a new child document instead of
expanding this file.
