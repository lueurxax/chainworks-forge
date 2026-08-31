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
  the two new terminal failure outcomes explicitly ineligible. Persisting the
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
FrozenEscalationTier { ledger_id, tier_id, backend_profile_id }
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

Migration adds immutable work-item column `execution_contract` with values
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

An existing retry or P058 escalation caused by some unrelated failure may
create a new exact attempt only through the typed occurrence/binding authority
path and a new linked exact `pending` work item. A Codex tier is never
activated by mutating or reusing a legacy work item after claim; if the current
ledger path cannot materialize that exact item before child launch, it fails
closed. Cloning a legacy payload/status is forbidden. The two failure kinds
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
without the required bounded complete state is not proof. Missing/duplicate
options, wrong-session notifications, limit overflow, ambiguous values,
configuration request send/read failure, malformed configuration responses,
provider rejection, or a final mismatch before `prompt_write_started` closes
the child and returns
`ACP_CODEX_EXACT_CONFIGURATION_REJECTED`; prompt-write-start count remains zero.
The adapter reports the bounded reason through the engine callback and waits
for `settle_exact_invocation_v1(configuration_rejected)` before returning the
work result. If the process dies between observation and settlement, startup
sees durable `configuration_started` and invokes that same settlement.

The ACP crate exposes a narrow callback interface but has no DB dependency.
Migration-owned table `exact_invocation_attempts` is keyed by
`(source_work_item_id, agent_execution_id)`, with unique generation ID and
persisted occurrence key/attempt number. One source work item may therefore
have multiple pre-prompt attempts, but each attempt has one idempotent row:

```text
claimed -> configuration_started -> prompt_write_started -> prompt_dispatched
claimed -> superseded | cancelled
configuration_started -> configuration_rejected
prompt_write_started | prompt_dispatched -> prompt_delivery_unknown
prompt_dispatched -> terminal_result_recorded
```

Claim inserts `claimed` before any child launch. The engine commits
`configuration_started` before launching the ACP child; failure to commit
launches nothing. Recovery may supersede and requeue only `claimed`. A crash or
recovery from `configuration_started` settles configuration rejection, because
exact configuration was not proved and the fenced writer was never available.
Recovery from `prompt_write_started` or `prompt_dispatched` settles prompt
delivery unknown. This is a narrow duplicate-prevention rule for exact
invocations, not the deferred general post-dispatch recovery contract.
`terminal_result_recorded` hands the provider result to existing output/result
settlement, which chooses completed or failed exact work without changing its
current output-contract semantics.

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

One CAS changes only that attempt from `configuration_started` to
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

Engine-owned `settle_exact_invocation_v1` is the sole terminal writer for both
`configuration_rejected` and `prompt_delivery_unknown`. One immediate,
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

Cancellation and retry-supersession use the same lock order. If they commit
first, arm returns no token and writer count is zero. If arm commits first,
cancellation records `prompt_delivery_unknown` while preserving its existing
canceled owner/Run outcome; retry supersession is refused and no replacement
is enqueued. Late provider output is quarantined and cannot reopen an exact
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
`P031RunStageTopologyOccurrenceReadModel` gains the same fields and does not
infer mode or task identity locally. Failure kind uses the retained raw
runtime-fact value, phase is a pure mapping for the two new transport codes,
and action uses the existing runtime-fact value.

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

GraphQL exposes:

```graphql
input DaemonGenerationInputV1 {
  endpoint: String!
  pid: String!
  startedAt: String!
  buildSha: String!
}

type DaemonGenerationV1 {
  endpoint: String!
  pid: String!
  startedAt: String!
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

Both generation input/output types contain required `endpoint` (1...2,048
UTF-8 bytes), positive decimal `pid`, the raw nonempty UTC RFC3339
`startedAt` value retained from status readback, and `buildSha` (0...128 visible
ASCII bytes; empty remains valid for a development build). Incomplete or
malformed status identity fails closed before probe. The probe compares the
parsed instant and other expected fields exactly with its serving process and returns
`DAEMON_GENERATION_CHANGED` instead of capability data on mismatch.

`generationToken` is lowercase 64-hex SHA-256 over bytes
`codex-model-variant-generation-v1\0`, then for each tuple field in the order
above a four-byte big-endian byte length followed by its UTF-8 bytes. It is not
an authorization token. Both V2 resolvers compare it with their own current
generation before loading run data and return `DAEMON_GENERATION_CHANGED` on
mismatch. Therefore a daemon replacement between probe and readback cannot
authorize a stale V2 document.

An actor-owned
`ModelVariantCapabilityCoordinator` keys state by exact daemon generation
`{ endpoint, pid, started_at, build_sha }` from current status readback. Its
closed states are `unknown`, `probing`, `compatible`, `incompatible`, and
`failed`; one single-flight probe exists per generation. Only an error-free
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
updates
`P031ActiveAgentExecutionReadModel` with nullable effort plus the closed mode.
Incompatible or failed state shows a blocking daemon-compatibility message and
does not send the document. That message can invoke the existing explicit
`Restart Daemon` operator command and warns that restart can interrupt active
work; it never invokes the command itself.

Run-detail loading is keyed by `{ run_id, generationToken, request_nonce }`.
Changing the selected Run or daemon generation immediately clears prior V2
rows, cancels the old task, and enters `unknown`/`probing`; a late response is
discarded unless all three keys still match. `unknown`, `probing`, `failed`,
and `incompatible` render distinct safe placeholders and never retain rows
from the previously selected Run.

`DAEMON_GENERATION_CHANGED` refreshes status and probes the new tuple once; it
never restarts the daemon. Timeout/decode `failed` state exposes a local
`Retry Readback` refresh button. A retry uses the same expected tuple with a
new nonce and remains single-flight; generation change returns to the normal
refresh path.

Capability handling never restarts or replaces the daemon automatically. The
existing explicit operator restart action remains the only restart authority.
After that action, the coordinator waits at most 30 seconds for a distinct
ready generation key, then probes it as new. It never transfers a
capability result across PID/start-time/build generations or interrupts active
work on its own. Old documents continue to work against the new daemon.

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
`P031ActiveAgentExecutionReadModel` values and has no terminal-failure fields;
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
- Non-Codex providers retain their existing requested-identity copy.
- Each rendered agent row owns one accessibility element. Its label contains
  agent and task, its value contains status plus the complete formatter output, and
  its hint describes only an existing action. Parent cards must not combine or
  hide occurrence accessibility children. Formatting must not change focus or
  selection.
- In Stages, `provider_configuration_rejected` renders `Configuration rejected`
  and `prompt_delivery_unknown` renders `Prompt delivery unknown`. When the
  retained raw action hint is `inspect_logs`, the row shows noninteractive text
  `Inspect daemon logs`; this slice adds no button, navigation destination, or
  accessibility action for it. The complete failure kind, phase, and raw action
  hint remain in the row's accessibility value. No generic retry copy is shown
  for either terminal failure. Overview remains running-only and does not claim
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
| Required configuration send/read failure | Same typed failure and bounded reap | Write not started |
| Restart from `configuration_started` | Same typed terminal settlement | Write not started |
| Invalid exact v2 provenance/request shape | Strict compile failure before child launch | Write not started |
| Prompt write fails after attempt begins | `ACP_PROMPT_DELIVERY_UNKNOWN` | Unknown; never reported as zero |
| Restart from armed/dispatched without terminal result | Same typed terminal settlement | Unknown; never requeued |
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

`ACP_PROMPT_DELIVERY_UNKNOWN` maps to new failure kind
`prompt_delivery_unknown`, `failure_kind_version = 2`, failure phase
`prompt_delivery`, output settlement `none`, existing hint `inspect_logs`, and
`retryable = false`. Its distinct transport code proves that the durable write
fence was committed and never records a zero-prompt assertion. The execution
enters the existing fresh-session quarantine/late-output isolation path and
places its stage or mediation owner in terminal operator hold, so possible
side effects or late outputs cannot be consumed by later automatic work.

Both failure kinds are ineligible for automatic retry, P058 escalation tiers,
P079 output repair, P086 resurrection, provider-health fallback, provider
switching, and weaker/default model selection. No retry ledger or new action
hint is introduced by this slice.

Persistence retains the raw new failure string in the existing bounded
`failure_kind_raw_debug` lane and maps it to `Unknown` for an old reader that
does not know version 2. The existing GraphQL `AgentFailureKind` enum remains
unchanged and likewise emits `UNKNOWN`; V2 readback carries the bounded raw
string separately. MCP and reports continue to expose their existing
nullable/string lanes. Because both rows use existing `inspect_logs`, old
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
   fallback. Every pair is re-resolved from frozen authority; payload-only,
   mismatched plan/binding/decision, duplicate-agent, and valid-pair
   substitution negatives reject before launch. A producer inventory requires
   every InvokeAgent enqueue site to classify as exact-aware or fail-closed.
6. Occurrence tests cover static frozen indices, P060 materialization identity,
   P017 owner identity, deterministic attempt order, and outputless tasks.
   Absent, empty, and nonempty outputs all allocate durable generations and
   execute `session/new`.
7. Shared byte-fixture tests cover every branch of
   `CodexConfigurationModeV1`, `TaskOccurrenceKeyV1`,
   `ExecutionBindingAuthorityV1`, and `SessionLaunchIntentV1`, plus duplicate,
   malformed, unknown-version/kind, missing/null, size, and cross-branch
   negatives. Missing intent defaults only for legacy mode.
8. Downgrade/lifecycle tests create exact-contract rows in every existing
   status with the new binary and run old claim/recovery/cancellation statements
   against the same DB. Triggered writes abort atomically, provenance/sequence
   remain unchanged, and child/prompt counts stay zero. Dedicated transition
   tests plus the checked SQL inventory cover claim, pre-arm recovery,
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
11. Attempt-state and configuration-rejection fault injection at every durable
    boundary proves: only pre-launch `claimed` may create a later attempt;
    `configuration_started` converges to one rejection; work/execution/
    generation/claim and real Stage/P017/Run states close atomically; prompt
    write count is zero; no AdvanceRun/retry/fallback is enqueued.
12. Arm race tests prove full ownership predicates and lock order against
    Run/Stage/P017 cancellation and retry supersession. Cancellation-first
    produces zero writes; arm-first produces one terminal unknown settlement.
    Short writes at every byte boundary and restart from armed/dispatched prove
    no requeue, no duplicate prompt, no repair/resurrection/fallback/escalation,
    and quarantine of late output.
13. Old-reader/new-row compatibility across DB, GraphQL, MCP, and reports,
    proving raw version-2 failure retention, old GraphQL enum value `UNKNOWN`,
    and existing `inspect_logs` hint.
14. GraphQL compatibility tests prove old active/topology fields and documents
    work unchanged on the new daemon and the new app never sends either V2
    document to an old daemon. The legacy topology resolver occurrence-matches
    exact static rows and excludes unrepresentable exact dynamic/P017 rows.
15. Capability tests cover exact expected/returned generation equality,
    canonical token bytes/digest, incomplete identity, false/missing/partial,
    timeout/decode, concurrent callers, same-generation manual retry,
    `DAEMON_GENERATION_CHANGED` refresh without restart, and explicit restart
    followed by a distinct ready generation within 30 seconds. Daemon A/B and
    Run A/B tests prove stale responses cannot populate current rows.
16. GraphQL tests prove both V2 paths use the shared classifier and occurrence
    join, derive from frozen/persisted authority rather than current catalog or
    payload, represent future lazy static rows with planned-only identity,
    include dynamic and owner-aware P017 rows after materialization, preserve
    deterministic attempt order, and expose terminal failures only in Stages.
17. Swift decoding, bounded-scalar, and formatter goldens for Sol, Terra, Luna,
    exact-looking legacy, generic legacy, missing effort, unknown bounded
    values, both terminal failure presentations, compact copy, full copy, and
    accessibility output.
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
- Configuration rejection and prompt-delivery-unknown settle terminally and
  visibly; retry, repair, resurrection, fallback, provider switch, and
  escalation do not react.
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
      keys and deterministic attempt order; P058 escalation and run-local
      health fallback use persisted frozen binding authority; payload-only or
      repeated-agent identity cannot authorize or cross-bind execution.
- [ ] Exact mode allocates fresh durable lineage with absent, empty, or nonempty
      outputs; typed P017 mediation resolves the frozen system-lead binding,
      while physical reuse, keep-alive, repair, resurrection, supplied live
      sessions, Steward, and every other non-stage owner reject before launch.
- [ ] Normative typed JSON/DB/Swift fixtures cover every exact mode,
      occurrence, binding-authority, and launch-intent branch and reject every
      malformed/unknown/cross-version shape.
- [ ] Exact-v1 work retains immutable provenance and DB-fenced transitions
      through claim/recovery/cancellation/settlement; pre-change claim and
      recovery cannot mutate or launch it and prompt write count remains at
      most one.
- [ ] Exact invocations verify model and effort in order before the first
      prompt, and every pre-prompt negative fixture proves zero prompt-write
      starts.
- [ ] Exact invocations use a fresh physical session and neither new failure
      enters automatic retry or escalation.
- [ ] Attempt-state fault injection proves configuration rejection atomically
      closes exact work, execution, generation, claim, and real Stage/P017/Run
      owner states with zero prompt writes and no generic AdvanceRun/retry.
- [ ] Full ownership-CAS race tests prove no writer access before the durable
      fence; cancellation-first writes zero bytes, while arm-first, every
      partial write, and armed/dispatched restart converge to one idempotent
      unknown settlement without startup requeue.
- [ ] Overview and Stages show the same friendly variant, effort, and `planned`
      qualifier and expose the complete exact model ID through the shared full
      value, help, and copy affordance.
- [ ] Active-agent effort is derived only from the frozen backend profile and
      remains nullable when unavailable.
- [ ] Old GraphQL shapes remain unchanged and exact rows cannot cross-bind in
      their legacy resolver; V2 exposes planned-only lazy identity plus
      materialized static/dynamic/P017 occurrence identity.
- [ ] Expected/returned generation equality plus the normative token binds the
      single-flight capability probe and both V2 documents to one daemon;
      mismatch refreshes/reprobes without restart and same-generation failure
      has a bounded manual retry.
- [ ] Capability failure never restarts a daemon automatically; only an
      operator action followed by a distinct ready generation permits re-probe.
- [ ] Typed configuration rejection cannot enter repair, resurrection,
      fallback, provider switching, or escalation.
- [ ] Old readers retain raw new failure values and decode the existing
      `inspect_logs` action; no new action-hint vocabulary is introduced.
- [ ] Separate Overview-running and Stages-occurrence presentation models keep
      terminal configuration states out of Overview and visible in Stages;
      complete model configuration is keyboard, pointer, and accessibility
      readable through row-local actions without moving selection or crossing
      windows.
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
