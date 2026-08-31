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
  the new pre-prompt configuration failure explicitly ineligible.
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
decoding; last-key-wins behavior is forbidden. Version 1 fixture bytes are
immutable after release; a vocabulary change requires a new fixture version
and policy ID.

The Rust workflow compiler embeds those exact fixture bytes and their SHA-256
digest and is the sole run-creation authority. The Xcode target bundles the
same file so `YAMLValidator` can provide preflight feedback, but Swift does not
compile or authorize a run. Missing, malformed, or digest-mismatched bundled
data is a visible validation error. The retained gate proves source bytes,
bundled bytes, and Rust-embedded bytes have the same digest.

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
    "fixture_sha256": "<64 lowercase hex characters>"
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
ExactVariantV1 { policy_id, fixture_sha256 }
```

It carries that mode through `ExecutionRequest`. `ExactVariantV1` requires a
valid compiler schema v2 marker, a Codex frozen binding whose pair exists in
the marked fixture, and `owner_kind == stage_execution`. It is valid for an
ordinary initial attempt or same-binding retry. A stage-scoped frozen fallback
binding may use it only when that fallback already names an approved exact
Codex pair; configuration rejection itself never selects that fallback.

Before launching a child, exact mode rejects `reuse_existing_session`,
`keep_session_alive`, a supplied provider-session or session-generation ID,
P079 repair paths, P086 attach/resurrection, Steward ownership, and every other
non-stage or session-reuse combination. The caller cannot downgrade an exact
v2 request to legacy mode. V1 snapshots retain their current legacy behavior.

### 3. Exact ACP configuration

For an exact-pair invocation one invocation-local
`CodexExactConfigurationHandshakeV1` actor owns the reader, option state, and
prompt writer. Its closed state machine is:

```text
awaiting_session_new -> setting_model -> setting_effort -> ready_to_prompt
any validation or transport failure --------------------------> rejected
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
10. Reverify the current in-memory pair immediately before the prompt write.
    The same actor, with no concurrent writer, then sends the first prompt.

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
malformed responses, send failure, provider rejection, or a final mismatch
closes the child and returns
`ACP_CODEX_EXACT_CONFIGURATION_REJECTED`; prompt-send count remains zero.

The in-memory option snapshot is invocation-local and discarded with the child;
it is not a durable acceptance receipt. Updates received after prompt dispatch
cannot create an accepted/configured UI claim because this slice exposes only
planned truth. The required path does not log option values, session
identifiers, or raw provider payloads. Existing bounded error/redaction
behavior remains in force.

### 4. Session lifetime

An exact-pair normal stage invocation owns one fresh physical Codex session. It
is not eligible for cross-invocation live-session reuse or P086 resurrection in
this slice. Retry creates another fresh session and repeats the complete
required configuration sequence.

This deliberate performance tradeoff avoids treating process-local or
historical evidence as durable acceptance. Efficient reuse is deferred to the
provider-acceptance child document and cannot be enabled by a flag.

Legacy v1 frozen invocations retain their current reuse behavior.

### 5. Readback

One server-side `PlannedProviderIdentityClassifierV1` derives planned identity
from the frozen snapshot for both readback paths. Its closed mode vocabulary
and precedence are:

1. non-Codex provider -> `not_applicable`;
2. missing or unreadable snapshot/profile binding -> `unavailable`;
3. valid compiler v2 marker, matching fixture digest, and supported exact pair
   -> `exact_variant_v1`;
4. compiler v1 or absent marker -> `legacy_best_effort_v0`, even when its raw
   model string happens to name an exact variant; and
5. compiler v2 with a missing, blank, generic, or unknown pair ->
   `unavailable` and fail-closed execution admission.

`runStageTopology.occurrences` remains the Stages source. Its existing model
and effort fields and new `configurationMode` are populated by that classifier.
`P031RunStageTopologyOccurrenceReadModel` gains the same closed mode and does
not infer it locally.

`activeAgentExecutions` stops returning the shared `GqlAgentExecution` object
and uses dedicated `GqlActiveAgentExecutionV1`. It adds nullable planned
`effort` and non-null `configurationMode`; model and effort are derived
together from the execution's `backend_profile_id` in the run's frozen
catalog. It does not read the current catalog or add/backfill a database
column. Every producer of this dedicated type uses the same helper.

GraphQL exposes `codexModelVariantReadbackV1: Boolean!`. Once per daemon
process generation and before selecting
`effort` or `configurationMode`, the app sends only this fixed capability
probe. An old daemon's unknown-field schema error triggers one bundled-daemon
replacement/restart and a second probe. If it is still absent, the app shows a
daemon incompatibility state and never sends the new run-detail document.
Capability success permits the versioned document and updates
`P031ActiveAgentExecutionReadModel` with nullable effort plus the closed mode.
This is lockstep compatibility, not silent nullable decoding.

No MCP, report, artifact, receipt, or runtime-health schema changes in this
slice.

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
  row's help text and context-menu action `Copy model configuration` expose the
  complete formatter output through standard macOS interaction.
- `planned` is mandatory. This slice never renders `accepted`, `configured`,
  `actual`, or equivalent claims.
- Missing effort renders `effort unavailable`; unknown nonempty values render
  bounded escaped text and never map to a known effort.
- Non-Codex providers retain their existing requested-identity copy.
- Each rendered agent row owns one accessibility element. Its label contains agent
  and task, its value contains status plus the complete formatter output, and
  its hint describes only an existing action. Parent cards must not combine or
  hide occurrence accessibility children. Formatting must not change focus or
  selection.

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

| Failure | Result | Prompt bytes |
|---|---|---:|
| Fresh matrix mismatch | Compile error; no run created | 0 |
| Exact model option absent/ambiguous | `ACP_CODEX_EXACT_CONFIGURATION_REJECTED` | 0 |
| Model set response lacks exact current value | Same typed failure | 0 |
| Effort option absent/ambiguous | Same typed failure | 0 |
| Final pair mismatch | Same typed failure | 0 |
| Required configuration transport failure | Same typed failure and bounded reap | 0 |
| Invalid exact v2 provenance/request shape | Same typed failure before child launch | 0 |
| Legacy generic frozen run | Existing legacy path; UI says unverified | Existing behavior |
| Active readback cannot resolve frozen profile | `effort = null`; UI says unavailable | No mutation |

`ACP_CODEX_EXACT_CONFIGURATION_REJECTED` maps to new domain failure kind
`provider_configuration_rejected`, failure phase `provider_configuration`,
output settlement `none`, and operator action hint `retry_same_binding` while
the existing same-binding budget remains, otherwise
`inspect_provider_configuration`. The failed AgentExecution and its zero-prompt
fact are immutable. Cleanup terminates and reaps the child within the existing
bounded cleanup policy.

This failure is ineligible for P079 output repair, P086 resurrection,
provider-health fallback, provider switching, and escalation to another
binding. It is retryable only through the existing bounded same-binding retry
budget; such a retry creates a new AgentExecution, process, and session and
repeats the full handshake. No failure falls back to a generic model, provider
default, weaker effort, or another provider.

## Verification gate

Add focused gate `codex-model-variant-slice`. It is provider-free and runs:

1. Strict fixture parsing, duplicate-key negatives, and SHA-256 parity across
   checked-in, Rust-embedded, and Xcode-bundled bytes.
2. Rust fresh compiler positives for every approved production row and bounded
   non-production fixture catalogs using supported exact pairs.
3. Mutation negatives for every production profile, generic model, unknown
   model/effort, Luna `ultra`, duplicate profile, missing canonical production
   profile, and undeclared extra production Codex profile.
4. Separate fresh and frozen admission tests: fresh compilation writes v2;
   every v1 snapshot remains byte-identical legacy even with an exact-looking
   pair; malformed v2 fails closed.
5. `ExecutionRequest` matrix proving exact mode rejects every reuse, keep-alive,
   repair, resurrection, supplied-session, and non-stage combination before
   child launch.
6. Fake ACP success proving exact request IDs, full-state replacement,
   dependent effort appearing only after model selection, notification
   interleaving, ordered model then effort configuration, and exactly one
   prompt after both exact `currentValue` checks.
7. Fake ACP negatives for alias-only, substring-only, duplicate, missing,
   malformed, empty-success, stale-snapshot, wrong-session, out-of-order,
   rejected, mismatched, and every numeric-limit overflow; each asserts zero
   prompt bytes and bounded child cleanup.
8. DB-backed settlement tests proving the typed failure, immutable failed
   attempt, no P079/P086/fallback dispatch, and a distinct fresh execution for
   an allowed bounded same-binding retry.
9. Capability tests with old-schema, successful replacement, and failed
   replacement fixtures, proving no new GraphQL document is sent before a
   successful probe.
10. GraphQL tests proving both active and topology paths use the same classifier,
    derive model/effort from the frozen profile rather than the current catalog,
    and classify exact-looking v1, valid v2, invalid v2, and missing bindings.
11. Swift decoding, bounded-scalar, and formatter goldens for Sol, Terra, Luna,
    exact-looking legacy, generic legacy, missing effort, unknown bounded
    values, compact copy, full copy, and accessibility output.
12. Hosted Overview and Stages tests at 292 points with `.large` and
    `.accessibility3` text proving friendly variant, effort, planned qualifier,
    and status remain distinguishable; full accessibility/copy values are
    available; focus and selection do not move; and parent accessibility does
    not hide rows.
13. Structural scans proving there is no feature flag, environment bypass,
    current-catalog read in run readback, or second formatter.

The gate fails when any selected Rust or Swift suite executes zero tests. It
does not invoke a live provider, network, remote UI host, or another proposal
gate.

## Rollout

- The approved matrix and exact ACP sequence become default behavior for every
  newly compiled run after release.
- There is no disable switch, experiment percentage, or operator opt-in.
- Pre-change frozen runs continue unchanged and visibly say legacy/unverified.
- A configuration rejection fails that attempt visibly. Only a bounded
  same-binding retry may react; repair, resurrection, fallback, provider switch,
  and escalation do not.
- Operational observation from a normal later run is useful but not required
  to merge this provider-free slice.

## Acceptance checklist

- [ ] The active catalog contains exactly the approved seven Codex pairs.
- [ ] The Rust compiler rejects every matrix mutation before run creation;
      Swift `YAMLValidator` reports the same mutations as non-authoritative
      preflight feedback.
- [ ] Fresh snapshots carry the validated v2 marker; every v1 snapshot,
      including exact-looking values, remains on typed legacy behavior.
- [ ] Existing generic frozen runs replay without byte mutation and without a
      fabricated Sol/Terra/Luna label.
- [ ] Exact mode rejects reuse, keep-alive, repair, resurrection, supplied
      sessions, and non-stage ownership before child launch.
- [ ] Exact invocations verify model and effort in order before the first
      prompt, and every negative fixture proves zero prompt bytes.
- [ ] Exact invocations use a fresh physical session; retry repeats the full
      sequence.
- [ ] Overview and Stages show the same friendly variant, effort, and `planned`
      qualifier and expose the complete exact model ID through the shared full
      value, help, and copy affordance.
- [ ] Active-agent effort is derived only from the frozen backend profile and
      remains nullable when unavailable.
- [ ] GraphQL capability negotiation prevents a new run-detail document from
      reaching an old daemon, and both readbacks expose the same typed mode.
- [ ] Typed configuration rejection cannot enter repair, resurrection,
      fallback, provider switching, or escalation.
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
| [Provider accepted truth and prompt authority](2026-08-31-provider-accepted-truth-and-prompt-authority-design.md) | Durable accepted configuration, stable task occurrence, reuse, prompt permits, delivery-unknown settlement, fallback ambiguity | P2-01 and accepted-truth portions of the checkpoint |
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
