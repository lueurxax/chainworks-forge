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
  the two new terminal failure outcomes explicitly ineligible.
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

Rust owns append-only `CodexModelVariantPolicyRegistryV1`. Its v1 entry pins
the policy ID, fixture path, byte length, and literal digest above. The compiler
embeds and resolves every retained registry entry, not only the newest one.
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

The engine derives a typed `CodexConfigurationMode` from frozen compiler
provenance, not from caller input:

```text
LegacyBestEffortV0
ExactVariantV1 { policy_id, fixture_sha256, backend_profile_id }
```

The engine loads the frozen plan by `run_id` and resolves the expected task
occurrence using durable `stage_execution_id`, `agent_execution_id`, agent ID,
and task identity. That occurrence supplies `backend_profile_id`; a persisted
frozen escalation tier supplies it only when the invocation was already routed
through that tier for some other failure. Queue payload values, including its
profile ID, are comparison evidence only. A missing occurrence/profile or any
payload/profile/provider/model/effort mismatch rejects before child launch; a
second valid matrix pair cannot be substituted. `ExecutionRequest` carries the
resolved `backend_profile_id` and typed mode, and the adapter rechecks the pair
against the pinned policy entry before `session/new`.

`ExecutionRequest` also carries typed `SessionLaunchIntentV1`:

```text
LegacyUnspecified
Fresh { lineage_generation_id }
Reuse { session_generation_id, provider_session_id }
```

Existing serialized requests without this field decode as
`LegacyUnspecified`, which is legal only with `LegacyBestEffortV0`; exact mode
requires an explicit fresh intent.

The fresh lineage ID is durable audit/output ownership and is not a live
transport handle. `ExactVariantV1` requires `Fresh`,
`owner_kind == stage_execution`, `reuse_existing_session == false`, and
`keep_session_alive == false`; the transport always launches a process and
executes `session/new`. It rejects `Reuse`, P079 repair paths, P086
attach/resurrection, Steward ownership, and every non-stage combination before
child launch. A normal stage with declared outputs retains its newly allocated
lineage ID through `Fresh` and does not bypass startup.

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
    The same actor, with no concurrent writer, transitions to
    `prompt_write_started` before the first write attempt and then sends the
    prompt.

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

Once `prompt_write_started` is set, no later error is a configuration
rejection and no zero-prompt claim is allowed. A complete write transitions to
`prompt_dispatched`. Any short/partial write, EPIPE, close, timeout, or other
error from the first write attempt returns
`ACP_PROMPT_DELIVERY_UNKNOWN`, failure kind `prompt_delivery_unknown`, and
output settlement `none`; it closes the physical session and is ineligible for
automatic retry, repair, resurrection, fallback, or escalation. This narrow
settlement prevents duplicate work without claiming durable provider
acceptance.

The in-memory option snapshot is invocation-local and discarded with the child;
it is not a durable acceptance receipt. Updates received after prompt dispatch
cannot create an accepted/configured UI claim because this slice exposes only
planned truth. The required path does not log option values, session
identifiers, or raw provider payloads. Existing bounded error/redaction
behavior remains in force.

### 4. Session lifetime

An exact-pair normal stage invocation owns one fresh physical Codex session. It
is not eligible for cross-invocation live-session reuse or P086 resurrection in
this slice. This slice does not authorize automatic retry of configuration or
prompt-delivery failures.

This deliberate performance tradeoff avoids treating process-local or
historical evidence as durable acceptance. Efficient reuse is deferred to the
provider-acceptance child document and cannot be enabled by a flag.

Legacy v1 frozen invocations retain their current reuse behavior.

### 5. Readback

One server-side `PlannedProviderIdentityClassifierV1` derives planned identity
from an already compiled frozen plan for both readback paths. Its closed mode
vocabulary and precedence are:

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

`runStageTopology.occurrences` remains the Stages source. Its existing model
and effort fields and new `configurationMode` are populated by that classifier.
`P031RunStageTopologyOccurrenceReadModel` gains the same closed mode and does
not infer it locally. Occurrences also expose nullable strings `failureKind`,
`failurePhase`, and `operatorActionHint`. Failure kind uses the retained raw
runtime-fact value, phase is a pure mapping for the two new transport codes,
and action uses the existing runtime-fact value; adding nullable fields does
not alter old documents.

Existing `activeAgentExecutions: [GqlAgentExecution!]!` and its resolver remain
unchanged for old applications, fragments, generated clients, and rollback.
New field `activeAgentExecutionsV2` returns dedicated
`GqlActiveAgentExecutionV2` with nullable planned `effort`, non-null
`configurationMode`, and the same nullable failure strings. It otherwise
preserves the old active field set and semantics. Model and effort are derived
together from the execution's `backend_profile_id` in the run's frozen catalog.
It does not read the current catalog or add/backfill a database column. Every
producer of this dedicated type uses the same helper.

GraphQL exposes `codexModelVariantReadbackV1: Boolean!`. An actor-owned
`ModelVariantCapabilityCoordinator` keys state by exact daemon generation
`{ endpoint, pid, started_at, build_sha }` from current status readback. Its
closed states are `unknown`, `probing`, `compatible`, `incompatible`, and
`failed`; one single-flight probe exists per generation. Only an error-free
response with `data.codexModelVariantReadbackV1 == true` is compatible. `false`,
missing data, unknown-field response, partial data with errors, timeout, and
decode failure never authorize the versioned document. `false`, missing data,
and unknown-field errors become `incompatible`; partial errors, timeout, and
decode failure become `failed`. The server returns true only when both
`activeAgentExecutionsV2` and topology identity/failure fields are installed.
A generation-key change invalidates prior state.

Before selecting `activeAgentExecutionsV2`, `effort`, or
`configurationMode`, the app completes that probe. Compatible state permits
the versioned run-detail document and updates
`P031ActiveAgentExecutionReadModel` with nullable effort plus the closed mode.
Incompatible or failed state shows a blocking daemon-compatibility message and
does not send the document. That message can invoke the existing explicit
`Restart Daemon` operator command and warns that restart can interrupt active
work; it never invokes the command itself.

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
- A focused-row `Copy Model Configuration` command in the app's Commands menu
  (`Option-Command-C`), its context-menu alias, and the row's named
  accessibility action invoke one copy handler. The command is disabled without
  a focused agent row, preserves focus and selection, and copies the complete
  untruncated formatter output.
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
- `provider_configuration_rejected` renders `Configuration rejected` plus
  existing action `Inspect logs`; `prompt_delivery_unknown` renders
  `Prompt delivery unknown` plus the same action. The complete failure kind,
  phase, and action hint remain in the row's accessibility value. No generic
  retry copy is shown for either terminal failure.

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
| Invalid exact v2 provenance/request shape | Strict compile failure before child launch | Write not started |
| Prompt write fails after attempt begins | `ACP_PROMPT_DELIVERY_UNKNOWN` | Unknown; never reported as zero |
| Legacy v1 frozen run | Existing legacy path; UI says unverified | Existing behavior |
| Valid plan cannot resolve execution profile | `effort = null`; UI says unavailable | No mutation |

`ACP_CODEX_EXACT_CONFIGURATION_REJECTED` maps to new domain failure kind
`provider_configuration_rejected`, `failure_kind_version = 2`, failure phase
`provider_configuration`, output settlement `none`, existing operator action
hint `inspect_logs`, and `retryable = false`. The failed AgentExecution and its
transport code are immutable; the adapter invariant and instrumented writer
prove that prompt write was not started. Cleanup terminates and reaps the child
within the existing bounded cleanup policy.

`ACP_PROMPT_DELIVERY_UNKNOWN` maps to new failure kind
`prompt_delivery_unknown`, `failure_kind_version = 2`, failure phase
`prompt_delivery`, output settlement `none`, existing hint `inspect_logs`, and
`retryable = false`. Its distinct transport code proves that the write attempt
started and never records a zero-prompt assertion. The execution enters the
existing fresh-session quarantine/late-output isolation path and places the
stage in terminal operator hold, so possible side effects or late outputs
cannot be consumed by later automatic work.

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
   synthetic later policy.
2. Rust fresh compiler positives for every approved production row and bounded
   non-production fixture catalogs using supported exact pairs.
3. Mutation negatives for every production profile, generic model, unknown
   model/effort, Luna `ultra`, duplicate profile, missing canonical production
   profile, and undeclared extra production Codex profile.
4. Separate fresh and frozen admission tests: fresh compilation writes v2;
   every v1 snapshot remains byte-identical legacy even with an exact-looking
   pair; malformed/unknown-policy v2 fails strict compilation and emits no
   partial planned-identity row.
5. Frozen-binding tests proving provider/model/effort are derived from
   `backend_profile_id`, substitution of another valid pair rejects before
   launch, and the adapter rechecks the same tuple.
6. `SessionLaunchIntentV1` matrix proving a normal output-declaring stage keeps
   fresh lineage and still executes `session/new`, while reuse, keep-alive,
   repair, resurrection, live-session handles, and non-stage combinations
   reject before launch; missing/`LegacyUnspecified` intent is accepted only on
   legacy mode.
7. Fake ACP success proving exact request IDs, full-state replacement,
   dependent effort appearing only after model selection, notification
   interleaving, ordered model then effort configuration, and exactly one
   prompt after both exact `currentValue` checks.
8. Fake ACP negatives for alias-only, substring-only, duplicate, missing,
   malformed, empty-success, stale-snapshot, wrong-session, out-of-order,
   rejected, mismatched, and every numeric-limit overflow; each asserts zero
   prompt-write starts and bounded child cleanup.
9. Short-write fixtures for every boundary from one byte through payload minus
   one proving `prompt_delivery_unknown`, no zero-prompt fact, immutable
   terminal settlement, and no automatic retry/repair/resurrection/fallback or
   escalation for either new failure; possible late output remains quarantined.
10. Old-reader/new-row compatibility across DB, GraphQL, MCP, and reports,
    proving raw version-2 failure retention, old GraphQL enum value `UNKNOWN`,
    and existing `inspect_logs` hint.
11. GraphQL compatibility tests proving the old field/type/document work
    unchanged on the new daemon and the new app never sends
    `activeAgentExecutionsV2` to an old daemon.
12. Capability-coordinator tests for error-free true, false, missing, partial
    errors, timeout, decode failure, concurrent callers, exact state mapping,
    generation change, no automatic restart, and an operator restart followed
    by a distinct ready generation within 30 seconds.
13. GraphQL tests proving both new active and topology paths use the same classifier,
    derive model/effort from the frozen profile rather than the current catalog,
    and classify exact-looking v1, valid v2, and missing bindings.
14. Swift decoding, bounded-scalar, and formatter goldens for Sol, Terra, Luna,
    exact-looking legacy, generic legacy, missing effort, unknown bounded
    values, both terminal failure presentations, compact copy, full copy, and
    accessibility output.
15. Hosted Overview and Stages tests at 292 points with `.large` and
    `.accessibility3` text proving friendly variant, effort, planned qualifier,
    and status remain distinguishable; full accessibility/copy values are
    available through keyboard command, context menu, and accessibility action;
    focus and selection do not move; and parent accessibility does not hide
    rows.
16. Structural scans proving there is no feature flag, environment bypass,
    current-catalog read in run readback, automatic capability-triggered daemon
    restart, replacement of the old GraphQL field, or second formatter.

The gate fails when any selected Rust or Swift suite executes zero tests. It
does not invoke a live provider, network, remote UI host, or another proposal
gate.

## Rollout

- The approved matrix and exact ACP sequence become default behavior for every
  newly compiled run after release.
- There is no disable switch, experiment percentage, or operator opt-in.
- Pre-change frozen runs continue unchanged and visibly say legacy/unverified.
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
- [ ] Exact mode preserves fresh durable lineage for ordinary output stages but
      rejects physical reuse, keep-alive, repair, resurrection, supplied live
      sessions, and non-stage ownership before child launch.
- [ ] Exact invocations verify model and effort in order before the first
      prompt, and every pre-prompt negative fixture proves zero prompt-write
      starts.
- [ ] Exact invocations use a fresh physical session and neither new failure
      enters automatic retry or escalation.
- [ ] Instrumented writer tests prove no write attempt for configuration
      rejection; every partial-write error settles as prompt delivery unknown
      without a zero-prompt claim.
- [ ] Overview and Stages show the same friendly variant, effort, and `planned`
      qualifier and expose the complete exact model ID through the shared full
      value, help, and copy affordance.
- [ ] Active-agent effort is derived only from the frozen backend profile and
      remains nullable when unavailable.
- [ ] The old GraphQL field/type remains unchanged; generation-bound,
      single-flight capability negotiation prevents the V2 document from
      reaching an old daemon, and both new readbacks expose the same typed mode.
- [ ] Capability failure never restarts a daemon automatically; only an
      operator action followed by a distinct ready generation permits re-probe.
- [ ] Typed configuration rejection cannot enter repair, resurrection,
      fallback, provider switching, or escalation.
- [ ] Old readers retain raw new failure values and decode the existing
      `inspect_logs` action; no new action-hint vocabulary is introduced.
- [ ] Terminal configuration states and complete model configuration are
      keyboard, pointer, and accessibility readable without moving selection.
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
| [Provider accepted truth and prompt authority](2026-08-31-provider-accepted-truth-and-prompt-authority-design.md) | Durable accepted configuration, stable task occurrence, reuse, prompt permits, general post-dispatch delivery settlement, fallback ambiguity; excludes this slice's first-write terminal safety classification | P2-01 and accepted-truth portions of the checkpoint |
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
