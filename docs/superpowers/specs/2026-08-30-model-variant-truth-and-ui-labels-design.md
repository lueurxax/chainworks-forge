# Codex Planned Variant Matrix and UI Labels

Date: 2026-08-30
Status: Draft; reduced hypothesis awaiting proposal-readiness approval
Source checkpoint: `f160bc12`

## Decision summary

This is the only active implementation proposal in the model-variant
decomposition. It tests one deliberately cheap hypothesis:

> If new Runs freeze a balanced, validated Codex model/effort matrix and the
> existing Overview and Stages rows render that frozen pair as `planned`, then
> operators can tell whether Sol, Terra, or Luna is assigned without adding a
> provider-truth subsystem or a new readback protocol.

The proposal changes catalog validation, the seven production Codex profiles,
the existing planned-value presentation, and focused provider-free tests. It
does not claim which model the remote provider ultimately used.

The previous exact-dispatch design is retained in Git for traceability. Its
dispatch ownership, accepted truth, crash recovery, bounded V2 readback, and
advanced UI contracts remain in the deferred documents under
[Decomposition](#decomposition). They are not prerequisites for this slice.

## Why this cut

The previous review showed that an atomic exact-at-prompt guarantee would need
provider-supported ordering, durable occurrence/reservation ownership,
crash-safe process supervision, and a new bounded readback protocol. Those
features do not cheaply test the operator hypothesis.

This revision therefore makes these choices explicit:

- `planned` means the frozen Chainworks request, not provider acceptance;
- no new execution state machine, DB table, migration, failure kind, process
  supervisor, prompt fence, or retry behavior is introduced;
- no GraphQL V2 root, capability probe, generation token, paging contract, or
  stable occurrence identifier is introduced;
- existing fan-out, owner-only, P017, P058, health fallback, cancellation,
  repair, resurrection, and process cleanup behavior is unchanged;
- existing Overview topology use, stacks, keyboard focus, and selection are
  preserved; filtering changes only to fail closed when no current stage is
  selected, and model/effort plus VoiceOver presentation change; and
- the active slice accepts that provider configuration can change after the
  last client observation. No UI or receipt may call the value accepted,
  configured, actual, or exact.

## Current baseline

- `examples/agents/agents.yaml` defines seven production Codex backend
  profiles, currently using generic `gpt-5.6`.
- Run snapshots already freeze each resolved agent's `provider`, `model`,
  `effort`, and backend profile.
- `runStageTopology.occurrences` already exposes planned provider, model, and
  effort from a verified frozen Run. It already returns an empty topology for
  snapshot-less or unparseable snapshot pairs.
- `activeAgentExecutions` already exposes execution identity, provider, and
  model; the existing topology map supplies frozen stage/task assignments.
- The Codex adapter already sends the requested model through its current
  Codex ACP compatibility path and sends effort separately through
  `session/set_config_option`.
- Stages currently renders raw copy such as
  `codex · gpt-5.6 · high`; Overview does not make the variant sufficiently
  clear.

## Goals

1. Assign one explicit GPT-5.6 variant and effort to every production Codex
   profile.
2. Reject a newly compiled production catalog when those seven rows differ
   from the approved matrix.
3. Freeze the authored full model ID and effort through existing RunPlan fields.
4. Preserve the full pair through the existing adapter request.
5. At regular text sizes, render the same friendly variant, full model ID,
   effort, and `planned` qualifier in Overview and Stages. Preserve the variant
   token visually and the complete value through help/accessibility at every
   supported text size.
6. Preserve verified old frozen Runs and existing lifecycle/recovery semantics
   without describing mutable snapshot-less fallback as frozen truth.
7. Enable the behavior unconditionally, with no feature flag or disable path.

## Non-goals

- Prove or persist provider-accepted, configured, or actual model/effort.
- Prevent a remote configuration change after Chainworks sends its request.
- Standardize all Codex ACP `session/new` fields or permission modes.
- Add a prompt fence, provider process supervisor, occurrence authority,
  reservation ledger, terminal receipt, or new failure classification.
- Change fan-out publication, Stage aggregation, cancellation, retry, P017,
  P058, health fallback, P079 repair, or P086 resurrection.
- Add or alter GraphQL roots, capability negotiation, generation tokens,
  topology paging, IDs, auth boundaries, MCP, reports, or Timeline.
- Add copy buttons, menus, inspector surfaces, settings, or runtime model
  selection.
- Backfill, migrate, or change execution behavior for snapshot-less legacy Runs.
- Introduce distinct UI row identity for simultaneous executions of one agent;
  that belongs to the deferred stable-occurrence/readback work.
- Change Claude, Gemini, Auggie, or Junie bindings.
- Require a live provider, network call, remote UI host, or dedicated
  Chainworks Run for merge evidence.

## Approved matrix

| Backend profile | Planned model | Effort | Role |
|---|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `max` | Cross-stage authority and hard decisions |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` | Architecture and contract review |
| `codex_audit_high` | `gpt-5.6-sol` | `ultra` | Read-only final audit |
| `codex_writer_high` | `gpt-5.6-terra` | `high` | Iterative proposal authoring |
| `codex_builder_high` | `gpt-5.6-terra` | `high` | General implementation |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `high` | Routine orchestration |
| `codex_ops_low` | `gpt-5.6-luna` | `high` | Bounded operations with the approved reasoning floor |

The historical `codex_ops_low` identifier remains stable. `low` and
`medium` stay parser-supported but are not approved for these production
profiles. Luna with `ultra` is invalid.

## Contract

### 1. Immutable policy fixture

Add `examples/agents/codex-model-variant-matrix.v1.json`. Its exact UTF-8
content is the following JSON plus one final LF:

```json
{
  "schema_version": 1,
  "policy_id": "codex_model_variant_matrix_v1",
  "provider": "codex_acp",
  "canonical_provider": "codex",
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

The fixture is 1,479 bytes and has SHA-256
`b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911`.
Version 1 is append-only. A later vocabulary adds a new file and policy ID
without replacing these bytes.

`domain::codex_model_variant_policy` defines both provider tokens: authored
catalog provider `codex_acp` and canonical resolved/request/readback provider
`codex`. A non-Codex provider with the same model ID never receives a Codex
label.

Its duplicate-aware `parse_policy_json_v1` rejects unknown fields, duplicate
keys or IDs, undeclared production models, unsupported efforts, and malformed
UTF-8 with typed `policy_schema_invalid` errors. The separate
`load_pinned_policy_v1` first enforces final LF, byte length, and literal digest
with typed `policy_bytes_mismatch`, then calls the parser. Workflow, ACP tests,
and Swift formatter tests consume this one policy source rather than
maintaining another matrix.

The Xcode project adds this exact file as an explicit app resource.
`CodexModelVariantPolicyLoaderV1` is the only Swift lookup authority. It loads
the named resource from `Bundle.main`, requires final LF, 1,479 bytes, and the
literal SHA-256 above using CryptoKit, then decodes the fixed schema. It returns
only `.available(policy)` or typed `.unavailable(reason)`; missing, unreadable,
truncated, malformed, length-mismatched, digest-mismatched, or undecodable data
all choose unavailable.

Both formatters require `.available`. When the loader is unavailable, every
Codex planned tuple renders `Planned assignment unavailable`; no friendly name,
effort, or `planned` qualifier is inferred. There is no hard-coded Swift fallback
matrix. Non-Codex existing copy remains unchanged. Rust remains run-creation
authority; Swift validation controls presentation only.

### 2. New-Run admission

Add typed workflow entrypoint `compile_for_new_run_v1`. `StartRun` is its only
daemon caller. The entrypoint opens and reads each workflow and catalog source
exactly once into owned byte buffers before any Run/work insertion transaction.
A duplicate-aware YAML loader parses each buffer once into an owned value while
its mapping visitor rejects repeated root or nested keys. Only after that check
does it typed-decode from the same owned value. Compilation and snapshot
serialization consume those decoded values rather than paths. The entrypoint
must not call the legacy path, re-open either path, or pre-scan one read and
decode another.

The resulting `NewRunAdmissionV1` owns the compiled plan and snapshot payloads
derived from those decoded values. It can be constructed only after
authored-catalog and canonical-plan validation. `StartRun` must receive it
before opening an insertion transaction. Any read, parse, decode, compile, or
policy failure writes zero Run, Stage, or work rows.

Admission enforces:

- the authored catalog contains all seven reserved Codex profile IDs exactly
  once and contains no other Codex profile;
- every reserved row byte-matches its policy model/effort and uses authored
  provider `codex_acp`; every resolved row uses canonical provider `codex`;
- generic `gpt-5.6`, unknown variants, and Luna `ultra` reject; and
- duplicate YAML mapping keys reject before typed decoding.

Non-Codex profiles remain governed by existing validation. Relaxed fixture
catalogs exist only behind `#[cfg(test)]` helpers that are absent from the daemon
binary. The focused source gate also verifies canonical
`examples/agents/agents.yaml` against the same exact-seven rule.

The raw catalog snapshot retains authored provider `codex_acp`. Existing
compiler canonicalization writes `codex` through the standard production path
to `ResolvedAgent.provider`, work payload, and `ExecutionRequest`; model,
effort, and backend profile remain byte-identical to the admitted row. Existing
fallback/retry producers may retain a known authored alias internally; Section
4 canonicalizes that alias at GraphQL readback. No new Run schema version, DB
column, or public execution contract is needed.

The policy check exists only in `compile_for_new_run_v1`. Existing `compile` and
`compile_from_snapshot_json` remain separate compatibility APIs and never
acquire current-matrix admission. Verified frozen snapshot pairs, including
generic or custom historical pairs, continue byte-for-byte. Snapshot-less
legacy fallback keeps its current mutable live-path recompilation behavior; it
is not byte-stable, receives no planned-label claim, and remains out of scope
for migration or repair. Existing empty-topology readback for such Runs remains
fail-closed.

There is no YAML switch, environment override, app preference, rollout flag,
or test-only production bypass. Non-production tests may construct a bounded
fixture catalog explicitly through test helpers; those helpers are not linked
into the daemon binary and are guarded by `#[cfg(test)]`.

### 3. Planned dispatch

For a new Run, every existing InvokeAgent producer continues through its
current owner, queue, retry, fan-out, and recovery path. This proposal adds no
producer branch.

The canonical production bridge is tested end to end for all seven rows:
`compile_for_new_run_v1 -> standard InvokeAgent work payload encode/decode ->
ExecutionRequest -> AcpSessionNewSpec -> serialized ACP requests`. It must retain
canonical provider `codex`, serialize the exact unsuffixed model in
`session/new.params.model`, and emit exactly one separate best-effort
`session/set_config_option` request for `reasoning_effort`. The effort is absent
from required config options. The bridge must not collapse values into
`model/effort`, substitute generic `gpt-5.6`, or case-fold a policy-known value.

`AcpSessionNewSpec` adds an internal `exact_best_effort_config_options` lane.
The Codex adapter places only policy-known `reasoning_effort` there. Transport
sends its admitted value byte-for-byte and never passes it through advertised
option, casing, name, description, or substring resolution. Existing fuzzy
best-effort options and required options for other adapters remain unchanged.

The adapter classifies every Codex request into exactly one effort lane. An
explicit `ExecutionRequest.effort` retains current precedence over a model
suffix. Classification examines raw model and explicit-effort bytes before
split/lowercase normalization; only the exact row below enters the exact lane.
Legacy values then retain current lowercase normalization before entering the
fuzzy lane.

| Input class | Lane | Serialized effort behavior |
|---|---|---|
| byte-exact bare policy model plus separate byte-exact allowed effort | exact best-effort | one request with admitted bytes |
| bare policy model plus unsupported, case-varied, or blank explicit effort | legacy fuzzy | one request with current normalized explicit value |
| literal generic or custom bare model plus explicit effort | legacy fuzzy | one request with current normalized explicit value |
| any combined `model/effort` with no explicit effort | legacy fuzzy | one request with current normalized suffix |
| any combined form plus explicit effort | legacy fuzzy | one request with normalized explicit value; suffix emits nothing |
| case-varied policy model, with any present effort | legacy fuzzy | one request with current normalized effective value |
| any bare model with `effort = nil` | none | no config request |

The default generic model follows the literal-generic rows. A trailing slash
without a suffix and no explicit effort follows `none`. No class may populate
both exact and fuzzy lanes, and `reasoning_effort` never enters
`required_config_options`.

A provider-free scripted NDJSON peer returns a normal `session/new` result,
then separately rejects the best-effort effort request with JSON-RPC
Method-not-found and a generic rejection. In both cases the existing transport
continues to the prompt; neither rejection is promoted to a required option or
provider-acceptance claim.

The current Codex ACP compatibility envelope, permission mode, process
lifetime, and error behavior remain unchanged. This proposal neither calls
that envelope fully standard nor treats a successful
`session/set_config_option` response as accepted truth. Protocol
standardization, provider-observed epochs, atomic configure-and-prompt, and
crash-safe dispatch ownership belong to
[Provider accepted truth and prompt authority](2026-08-31-provider-accepted-truth-and-prompt-authority-design.md).

Any provider fallback, silent default, or later remote configuration change is
outside the truth represented here. It may affect actual execution, but it
must not rewrite the frozen planned pair or change UI copy to
`configured`, `accepted`, `actual`, or `exact`.

### 4. Existing readback only

No GraphQL schema changes are required:

Before compiling a stored snapshot, engine and GraphQL use one pure
`workflow::snapshot_integrity::verify_complete_pair_v1` authority. It accepts
the workflow JSON, catalog JSON, and both stored SHA-256 values and returns only
`Absent`, `Verified(pair)`, or typed `Invalid(reason)`. `Verified` requires all
four nonblank fields, canonical 64-character lowercase hash text, and exact
digests of the stored UTF-8 JSON. The existing engine helper delegates to this
authority rather than retaining another verifier.

`runStageTopology` and the plan-enrichment part of `activeAgentExecutions` both
compile only `Verified` pairs. `Absent`, partial/blank fields, malformed hash,
digest mismatch, tampered JSON, or compile failure fail closed to empty topology
and no active-row plan enrichment. Active execution rows may retain existing raw
runtime fields, but they cannot receive a planned label from invalid evidence.

- Stages uses the existing `runStageTopology.occurrences` provider, model,
  and effort values. An empty topology, including snapshot-less legacy, adds no
  planned label.
- `planned` is one provider/model/effort tuple from one frozen run-plan
  occurrence. Overview uses the existing StageExecution-to-stage mapping plus
  active agent ID to select candidates, then requires exactly one occurrence
  whose canonical provider and model byte-match the active execution. All
  three displayed planned fields come from that occurrence; the active row may
  select and verify it but may not donate one field.
- A unique canonical-provider/model match with absent effort renders the known
  model plus `Planned effort not recorded`. No candidate, multiple candidates,
  provider/model mismatch, owner-only work, or an unrepresented P017/dynamic
  occurrence renders `Planned assignment unavailable`. The existing raw
  execution model may remain separately visible without a friendly label,
  effort, or `planned` qualifier.
- Health fallback and P058 that change the binding therefore cannot combine a
  target execution model with source topology effort. A same-binding retry may
  display planned identity only when the unique occurrence still matches.
- Before GraphQL construction, the `activeAgentExecutions` resolver asks
  `ProviderFamily::resolve` whether a provider belongs to the Codex family. Only
  recognized Codex aliases emit canonical `codex`; every Claude, Gemini, Auggie,
  Junie, and unknown provider string remains byte-identical to existing readback.
  This normalizes Codex standard, health-fallback, P058/backend-profile,
  same-backend retry, and targeted-retry rows without rewriting frozen snapshots
  or internal rows. The normalization runs only inside
  `p093_active_agent_executions` after shared conversion; it does not change
  `GqlAgentExecution::from` or the stage-scoped `agentExecutions` query.
- Overview filtering changes narrowly: when a current topology stage exists,
  only active rows whose resolved StageExecution maps to that exact stage are
  visible; when none exists, the current-stage agent card is empty. Stale,
  unresolved, mapped-noncurrent, and completed rows stay hidden and are never
  reassigned merely to show unavailable copy.
- This slice adds no row and changes no row key. If the existing presentation
  cannot represent simultaneous executions of one agent distinctly, it does
  not claim to fix or validate that baseline case. Ambiguous repeated topology
  candidates receive unavailable only on rows the existing view already
  represents.

The change must not alter root queries, authorization, topology stacks,
occurrence identity, keyboard focus, selection, pagination, or stale-response
handling beyond the fail-closed current-stage filter above. It must not add a
second query or broaden data access.

### 5. Shared presentation

One pure Swift formatter receives provider, frozen model, and frozen effort. It
returns a bounded enum-backed presentation with optional nontruncating
`variantToken`, truncatable `visualSuffix`, and `fullAccessibilityValue`. Their
complete policy-known assembly is:

```text
Sol · gpt-5.6-sol · max · planned
Terra · gpt-5.6-terra · high · planned
Luna · gpt-5.6-luna · high · planned
```

The corresponding full help/accessibility values retain provider context, for
example `Codex · Sol · gpt-5.6-sol · max · planned`.

Recognized legacy and unavailable values remain honest:

```text
Codex · gpt-5.6 · high · planned
Codex · Sol · gpt-5.6-sol · Planned effort not recorded
Planned assignment unavailable
```

| State | `variantToken` | `visualSuffix` | `fullAccessibilityValue` |
|---|---|---|---|
| unique policy-known tuple with allowed effort | `Sol`, `Terra`, or `Luna` | full ID, effort, `planned` | `Codex`, friendly name, full ID, effort, `planned` |
| unique literal legacy `gpt-5.6` with parser-known effort | `Codex` | full ID, effort, `planned` | complete assembled value |
| unique policy-known or literal legacy model, effort absent | friendly name or `Codex` | model then `Planned effort not recorded` | provider, friendly/model, and the complete phrase |
| unknown model, unknown nonempty effort, no/ambiguous/mismatched tuple | absent | `Planned assignment unavailable` | same phrase |
| stale/unmapped active row | absent | hidden by existing filtering | absent |
| non-Codex | existing provider copy unchanged | existing suffix unchanged | existing accessibility copy unchanged |

Rules:

- friendly names are byte-exact lookup labels, never proof of provider use;
- policy-known output puts only the short friendly variant in `variantToken`.
  The suffix starts with the full model ID and contains effort plus `planned`;
- for `.xSmall ... .large`, the complete policy-known visual assembly is
  required. For `.xLarge ... .accessibility5`, the complete `variantToken`
  remains visible while only `visualSuffix` may tail-truncate. Help and
  VoiceOver always use `fullAccessibilityValue`;
- no new button, menu, shortcut, selection behavior, or topology ownership is
  introduced;
- missing effort and missing assignment use the two distinct normative phrases
  in the table;
- unknown/custom model or effort strings are never interpolated into the new
  planned line. Existing raw fields may retain their current presentation, but
  receive no friendly name or planned qualifier; and
- non-Codex providers retain current copy.

Overview and Stages must call the same formatter. Source scans reject local
Sol/Terra/Luna switch statements elsewhere in the app.

Both surfaces keep their current one-line metadata geometry. For a Codex planned
presentation they render `variantToken` as a separate plain `Text` with fixed
horizontal size and higher layout priority, followed by one tail-truncating
`Text` containing `visualSuffix` before task/status/session suffixes. No capsule,
badge, or font-size override is added.

`P036StageTopologyMetrics.cardWidth = 292` and `cardHeight = 210` describe the
inner frame before the existing 12-point outer padding; the measured one-unit
outer border is therefore 316 by 234. For `heightUnits = n`, inner height remains
`n * 210 + (n - 1) * 12`, with 24 points added by outer padding. Existing
connector segments remain 34 by 210 with 12-point column spacing. This slice
adds no vertical line and preserves these measured frames, Overview row height,
selection, and keyboard focus. It does not claim to repair the baseline
outer-card/connector asymmetry or whole-card Dynamic Type reflow; those remain
in [Verified provider truth UI](2026-08-31-verified-provider-truth-ui-design.md).

Overview's combined accessibility label includes the full planned value. The
Stages card stops overriding all occurrence children with one card label: its
header keeps a combined stage summary, while each existing occurrence row is a
separate accessibility child with task/status and the full planned value. This
changes only VoiceOver containment; it adds no visual row, command, or selection
state.

The normative nonempty-component order is:

- Overview help: `fullAccessibilityValue` exactly;
- Overview accessibility label: agent title, status, full planned value, stage,
  task, session, event count;
- Stage occurrence help: `fullAccessibilityValue` exactly; and
- Stage occurrence accessibility label: agent title, task, status, full planned
  value, execution count.

Components use comma-space separators and omit absent suffixes. The Stage header
summary contains no occurrence planned value, so every represented occurrence
exposes that value exactly once in the accessibility tree.

## Failure behavior

| Condition | Result |
|---|---|
| New production catalog differs from the policy | compilation fails before Run creation |
| Rust policy fixture missing, malformed, or digest-mismatched | new-Run compilation fails before writes |
| Swift bundle fixture missing, unreadable, malformed, length/digest-mismatched, or undecodable | both surfaces render `Planned assignment unavailable`; no fallback matrix |
| Verified old frozen Run contains literal generic model | replay unchanged; recognized generic planned line may be shown |
| Verified old frozen Run contains custom model/effort | replay unchanged; new planned line is unavailable and existing raw field remains unqualified |
| Snapshot-less legacy Run | current mutable recompilation behavior; topology remains empty and no planned label is added |
| Snapshot quartet absent, partial, malformed, tampered, digest-mismatched, or unparseable | topology is empty; active rows receive no plan enrichment or planned label |
| Overview has no unique matching frozen occurrence | `Planned assignment unavailable`; raw execution model may remain separately unqualified |
| Overview has no current topology stage | current-stage agent card is empty; no global fallback list |
| Provider advertises conflicting spelling/substrings for effort | exact admitted effort is still sent once as best-effort |
| Known Codex fallback/retry alias reaches readback | GraphQL emits `codex`; every non-Codex/unknown provider remains byte-identical and unlabelled by this formatter |
| Provider ignores or later changes the request | existing runtime behavior; planned UI remains planned |
| Unknown model/effort reaches formatter | `Planned assignment unavailable`; unknown text is not interpolated |

No new retry, fallback, cancellation, failure-kind, operator-action, or
quarantine behavior is introduced.

## Verification gate

Add focused gate `codex-planned-variant-slice`. It is provider-free and runs
only the following:

1. Policy-parser tests exercise duplicate keys, unknown fields, duplicate IDs,
   malformed rows, unsupported effort, generic production model, and Luna
   `ultra` through `parse_policy_json_v1`. Pinned-loader tests independently
   exercise LF/length/digest mutation and recompute the pinned bytes.
2. New-run loader tests prove each source is read once; duplicate root and
   nested YAML keys fail before typed decoding from the checked value. Admission
   mutation-tests every model, effort, authored/canonical provider, duplicate,
   missing profile, and extra Codex profile. Every `StartRun` failure writes zero
   Run/Stage/work rows, and relaxed helpers are absent from the daemon build.
3. The shared snapshot verifier covers a valid quartet, all-absent, every
   partial/blank combination, malformed hash text, tampered workflow/catalog
   JSON, digest mismatch, and compile failure. Both GraphQL paths fail closed to
   empty topology/no plan enrichment. Verified replay remains byte-identical; a
   snapshot-less fixture proves mutable recompilation plus no planned label.
4. A provider-free table carries all seven rows through compile, production
   payload encode/decode, `ExecutionRequest`, and `AcpSessionNewSpec`, then
   inspects serialized `session/new.params.model` and the single best-effort
   `reasoning_effort` request. Empty, case-variant, missing, and
   substring-conflicting advertised options cannot replace the admitted effort.
   A scripted peer rejects it with Method-not-found and generic error in separate
   cases; both still observe the prompt and no required effort option. The closed
   compatibility table additionally covers policy/unsupported, literal generic,
   custom, combined, case-varied, blank, explicit-over-suffix, trailing-slash,
   and absent classes. Each asserts exactly one lane, `AcpSessionNewSpec`
   membership, serialized request count/value, and no required effort option.
5. Resolver-local GraphQL tests prove `p093_active_agent_executions` alone emits
   `codex` for Codex standard, health-fallback, P058/backend-profile,
   same-backend retry, and targeted-retry aliases. Stored provider, snapshot
   bytes/hashes, shared `GqlAgentExecution::from`, stage-scoped
   `agentExecutions.provider`, every non-Codex alias, and unknown providers remain
   byte-identical. Swift tests cover
   unique/no/multiple candidates, ambiguous repeated topology, owner-only,
   dynamic, health fallback, P058 same/changed binding, lead/P017 exclusion, and
   provider/model mismatch.
   Current-stage filtering fixtures cover mapped-current visible,
   mapped-noncurrent hidden, unresolved hidden, no-current-stage empty, completed
   hidden, and no cross-stage reassignment. No new root, field, document, ID, or
   schema snapshot is added; no test claims distinct simultaneous same-agent rows.
6. Swift unit tests prove every visual/accessibility table row, Sol/Terra/Luna,
   literal generic, missing effort, unknown model/effort, and every control
   class in Overview and Stages. Delimiter, bidi, default-ignorable,
   line-separator, literal-escape, and overlong unknown inputs all produce the
   fixed unavailable phrase and are never interpolated.
7. Swift loader tests inject missing, unreadable, truncated, malformed,
   length/digest-mismatched, and undecodable resource data and assert unavailable
   on both surfaces. A built-app resource test reads exactly 1,479 bytes and the
   pinned digest from `Bundle.main`; source scans reject a fallback matrix.
8. Provider-free in-process `NSHostingView` tests measure 292-by-210 inner,
   316-by-234 one-unit outer, formula-derived multi-unit, and 34-by-210 connector
   frames for 1/2/5 occurrences before and after the change. `.xSmall ... .large`
   shows the complete longest assembly; `.xLarge ... .accessibility5` preserves
   the separate variant token while only the suffix truncates. Planned-row
   bounds, keyboard focus, row identity, selection, and status-only geometry do
   not regress; no whole-card baseline Dynamic Type conformance is claimed.
   Hosted inspection also proves the Overview combined label and actual macOS
   accessibility tree expose each Stage occurrence's full planned value exactly
   once in normative order after status-only refresh.
9. Static scans prove there is no feature flag/disable path and no second
   variant lookup table.

The gate fails if any selected Rust or Swift test filter executes zero tests.
It runs through `scripts/test-gate.sh`; no live Codex process or remote UI host
is part of acceptance.

## Rollout

- Update the seven catalog rows and formatter in one implementation change.
- New Runs receive the matrix automatically after deployment.
- Existing Runs with verified frozen snapshots continue without migration.
  Snapshot-less Runs retain current mutable fallback and receive no planned
  label from this slice.
- There is no enablement phase, kill switch, or disabled-by-default state.
- Rollback reverts the implementation commit. Runs already frozen with named
  variants remain readable as raw planned values.
- The next ordinary product Run may provide opportunistic visual evidence, but
  no dedicated provider run is required to merge this slice.

## Acceptance checklist

- [ ] The policy fixture is byte-pinned, append-only, and parsed by one strict
      Rust authority with distinct schema and byte-integrity failures.
- [ ] The production catalog contains exactly the seven approved pairs.
- [ ] Single-read typed new-Run admission rejects duplicate YAML and every
      exact-seven matrix mutation with zero Run/Stage/work writes; verified
      frozen snapshots replay unchanged and snapshot-less behavior is explicit.
- [ ] One shared complete-quartet verifier gates engine and both GraphQL plan
      reads; every absent/partial/tampered/mismatched/invalid pair fails closed.
- [ ] Authored `codex_acp` canonicalizes to `codex`; the provider-free production
      bridge proves serialized model, one byte-exact best-effort effort request
      despite conflicting advertised options, and prompt continuation after
      effort rejection without claiming acceptance.
- [ ] Every Codex effort input class enters exactly one exact/fuzzy/none lane;
      admitted rows are exact, declared legacy behavior remains fuzzy, absent
      emits none, and effort never becomes required.
- [ ] Only active-agent resolver-local Codex fallback/retry aliases emit `codex`;
      storage, snapshots, stage-scoped GraphQL, and non-Codex bytes stay unchanged.
- [ ] The byte-pinned Swift bundle loader is the only label authority and every
      resource failure renders unavailable without a fallback matrix.
- [ ] Overview and Stages use the same formatter and, at regular size, visibly
      show friendly variant, full ID, effort, and `planned` only from one frozen
      tuple; the variant stays visible and the complete value remains available
      through help/VoiceOver at all supported sizes.
- [ ] No/ambiguous/mismatched assignments and missing effort use distinct
      normative copy; unknown values are never interpolated into planned copy.
- [ ] Existing topology stacks, identities, keyboard focus, authorization,
      retries, fan-out, recovery, and provider lifecycle are unchanged. The
      current-stage filter fails closed, VoiceOver exposes planned copy exactly
      once per represented occurrence, and simultaneous same-agent row identity
      remains explicitly deferred.
- [ ] Existing measured inner/outer/connector geometry is preserved; complete
      copy is required only through `.large`, while the friendly variant token
      and full help/VoiceOver value survive all supported accessibility sizes.
- [ ] No public surface says configured, accepted, actual, or exact.
- [ ] No feature flag or disable path exists.
- [ ] `./scripts/test-gate.sh codex-planned-variant-slice` passes with
      nonzero Rust and Swift test counts.

## Decomposition

These deferred documents preserve removed scope. Each needs its own
sub-2,000-line design, review, implementation, and closeout cycle:

| Deferred child | Ownership removed from this slice |
|---|---|
| [Provider accepted truth and prompt authority](2026-08-31-provider-accepted-truth-and-prompt-authority-design.md) | provider-observed/atomic configuration, occurrence and reservation authority, prompt fence, supervisor, terminal receipts, exact failure/recovery |
| [Provider configuration migration and reconciliation](2026-08-31-provider-configuration-migration-and-reconciliation-design.md) | registry migration, reconciliation, bootstrap manifests |
| [P079 repair output materialization](2026-08-31-p079-repair-output-materialization-design.md) | repair staging, leases, activation, purge |
| [P086 resurrection containment](2026-08-31-p086-resurrection-containment-design.md) | attach/resurrection containment and output-only recovery |
| [Provider egress and diagnostics containment](2026-08-31-provider-egress-and-diagnostics-containment-design.md) | endpoint, TLS/DNS/redirect, egress, diagnostics |
| [P031 bounded runtime readback](2026-08-31-p031-bounded-runtime-readback-design.md) | new bounded GraphQL operations, errors, generation capability, paging, readback IDs |
| [Frozen run replacement and input repair](2026-08-31-frozen-run-replacement-and-input-repair-design.md) | replacement API and repair workspace |
| [Verified provider truth UI](2026-08-31-verified-provider-truth-ui-design.md) | configured/accepted states, advanced interactions, Timeline/Inspector |

Deferred inventories are not implementation authority and do not block this
planned-label hypothesis.

## Scope-budget check

This active specification must remain below 2,000 physical lines. At or above
that threshold, refinement stops and an independent responsibility moves to a
child document before review continues.
