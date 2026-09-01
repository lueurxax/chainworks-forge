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
- existing Overview topology use, stacks, filtering, focus, and selection are
  preserved; only model/effort formatting changes; and
- the active slice accepts that provider configuration can change after the
  last client observation. No UI or receipt may call the value accepted,
  configured, actual, or exact.

## Current baseline

- `examples/agents/agents.yaml` defines seven production Codex backend
  profiles, currently using generic `gpt-5.6`.
- Run snapshots already freeze each resolved agent's `provider`, `model`,
  `effort`, and backend profile.
- `runStageTopology.occurrences` already exposes planned provider, model, and
  effort from the frozen Run.
- `activeAgentExecutions` already exposes execution identity and model; the
  existing topology map supplies frozen stage/task assignments.
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
5. Render the same friendly variant, full model ID, effort, and `planned`
   qualifier in Overview and Stages.
6. Preserve old frozen Runs and all existing execution/recovery behavior.
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

The Xcode bundle contains the same retained fixture for validation and
presentation lookup. Rust remains the run-creation authority; Swift validation
is feedback only.

### 2. New-Run admission

Add typed workflow entrypoint `compile_for_new_run_v1`. It first performs the
existing fresh YAML compilation, then validates the authored catalog and
canonical resolved plan before returning `NewRunAdmissionV1`. `StartRun` is the
only daemon caller and must receive this value before opening any Run/work
insertion transaction. A validation failure writes zero Run, Stage, or work
rows.

Admission enforces:

- every Codex profile uses a declared model and allowed effort;
- each of the seven reserved production profile IDs, when present,
  byte-matches its policy row and uses provider `codex_acp`;
- generic `gpt-5.6`, unknown variants, and Luna `ultra` reject; and
- duplicate YAML mapping keys reject before typed decoding.

The focused source gate separately requires canonical
`examples/agents/agents.yaml` to contain all seven reserved rows exactly once
and no additional Codex profile. Bounded test catalogs may use other profile
IDs with supported pairs; they cannot impersonate or weaken a reserved row.

The raw catalog snapshot retains authored provider `codex_acp`. Existing
compiler canonicalization writes `codex` to `ResolvedAgent.provider`, work
payloads, `ExecutionRequest`, and GraphQL readback; model, effort, and backend
profile remain byte-identical to the admitted row. No new Run schema version,
DB column, or execution contract is needed.

The policy check exists only in `compile_for_new_run_v1`. Existing `compile`,
`compile_from_snapshot_json`, snapshot-less legacy fallback, and catalog
retrofit keep their current compatibility behavior and never acquire current
matrix admission. Old generic or custom Runs therefore replay byte-for-byte.

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
ExecutionRequest -> AcpSessionNewSpec`. It must retain canonical provider
`codex`, the exact unsuffixed model, and exactly one separate best-effort
`reasoning_effort` config option. It must not collapse values into
`model/effort`, substitute generic `gpt-5.6`, or case-fold a policy-known
value. The existing nonfatal behavior when that effort option is unsupported
or rejected remains unchanged.

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

- Stages uses the existing `runStageTopology.occurrences` provider, model,
  and effort values.
- `planned` is one provider/model/effort tuple from one frozen run-plan
  occurrence. Overview uses the existing StageExecution-to-stage mapping plus
  active agent ID to select candidates, then requires exactly one occurrence
  whose canonical provider and model byte-match the active execution. All
  three displayed planned fields come from that occurrence; the active row may
  select and verify it but may not donate one field.
- No candidate, multiple candidates, missing effort, provider/model mismatch,
  owner-only work, or an unrepresented P017/dynamic occurrence renders
  `Planned assignment unavailable`. The existing raw execution model may remain
  separately visible without a friendly label, effort, or `planned` qualifier.
- Health fallback and P058 that change the binding therefore cannot combine a
  target execution model with source topology effort. A same-binding retry may
  display planned identity only when the unique occurrence still matches.

The change must not alter root queries, authorization, topology stacks,
filtering, occurrence identity, focus, selection, pagination, or stale-response
handling. It must not add a second query or broaden data access.

### 5. Shared presentation

One pure Swift formatter receives provider, frozen model, and frozen effort and
returns `compact` visible copy plus `full` help/accessibility copy. For known
Codex IDs both are identical:

```text
Codex · Sol · gpt-5.6-sol · max · planned
Codex · Terra · gpt-5.6-terra · high · planned
Codex · Luna · gpt-5.6-luna · high · planned
```

Legacy and unavailable values remain honest:

```text
Codex · gpt-5.6 · high · planned
Codex · Sol · gpt-5.6-sol · Planned effort not recorded
Planned assignment unavailable
```

| State | Compact planned line | Full help/accessibility value |
|---|---|---|
| unique known tuple | friendly name, full ID, effort, `planned` | identical complete line |
| unique generic tuple | raw model, effort, `planned` | identical complete line |
| unique unknown tuple | bounded compact raw model, effort, `planned` | bounded full raw model, effort, `planned` |
| unique tuple, effort absent | model plus `Planned effort not recorded` | identical complete line |
| no/ambiguous/stale/mismatched tuple | `Planned assignment unavailable` | same phrase; no inferred pair |
| non-Codex | existing provider copy | unchanged |

Rules:

- friendly names are byte-exact lookup labels, never proof of provider use;
- the full model ID, effort, and `planned` qualifier remain visible in both
  Overview and Stages;
- the compact row may wrap but must not overlap, hide the variant, or change
  card dimensions while status updates;
- existing help/accessibility text uses the formatter's full output;
- no new button, menu, shortcut, selection behavior, or topology ownership is
  introduced;
- missing effort and missing assignment use the two distinct normative phrases
  in the table;
- unknown nonempty values trim ASCII edge whitespace and escape C0/C1/DEL and
  line breaks as uppercase `\u{HEX}`. The complete compact line is capped at
  64 UTF-8 bytes and the full line at 96, both including ASCII suffix
  `...[truncated]`; each cut ends on an extended grapheme-cluster or complete
  escape boundary; and
- non-Codex providers retain current copy.

Overview and Stages must call the same formatter. Source scans reject local
Sol/Terra/Luna switch statements elsewhere in the app.

At the existing 292-point card width, the regular metadata region reserves two
wrapped compact lines; accessibility text categories use a deterministic
four-line region. Height stays stable within a category. The 64-byte compact
boundary, longest known value, and status refresh may not clip, overlap,
resize within that category, reorder the agent/status/planned accessibility
value, or change row identity/focus. Full copy remains available through
existing help and accessibility without forcing the visible row to render 96
bytes.

## Failure behavior

| Condition | Result |
|---|---|
| New production catalog differs from the policy | compilation fails before Run creation |
| Policy fixture missing, malformed, or digest-mismatched | compilation fails; Xcode validation reports the same issue |
| Old frozen Run contains generic or custom model | replay unchanged; raw planned value is shown |
| Overview has no unique matching frozen occurrence | `Planned assignment unavailable`; raw execution model may remain separately unqualified |
| Provider ignores or later changes the request | existing runtime behavior; planned UI remains planned |
| Unknown model/effort reaches formatter | bounded escaped raw value, never a friendly false match |

No new retry, fallback, cancellation, failure-kind, operator-action, or
quarantine behavior is introduced.

## Verification gate

Add focused gate `codex-planned-variant-slice`. It is provider-free and runs
only the following:

1. Parser tests exercise duplicate keys, unknown fields, duplicate IDs,
   malformed rows, unsupported effort, generic production model, and Luna
   `ultra` through `parse_policy_json_v1`. Loader tests independently exercise
   LF/length/digest mutation and recompute the pinned bytes.
2. New-Run admission tests mutation-test every model, effort, authored/canonical
   provider, duplicate, missing profile, and extra Codex profile. `StartRun`
   failures write zero Run/work rows; its test-only constructors are absent
   from the daemon build.
3. Frozen replay, snapshot-less legacy fallback, and catalog retrofit tests
   prove old generic/custom behavior remains byte-identical and bypasses
   current-policy admission.
4. A provider-free table test carries all seven rows through canonical compile,
   production work payload encode/decode, `ExecutionRequest`, and
   `AcpSessionNewSpec`, asserting unsuffixed model, one separate effort, no
   generic substitution, and unchanged nonfatal option rejection.
5. Existing GraphQL tests prove topology still returns one frozen tuple. Swift
   tests cover unique/no/multiple matches, repeated tasks, owner-only, dynamic,
   health fallback, P058 same and changed binding, lead/P017 exclusion, stale
   stage mapping, and provider/model mismatch. No new root, field, document, or
   schema snapshot is added.
6. Swift unit tests prove every visual/AX table row, Sol/Terra/Luna, generic,
   unknown, every control class, exact 64/96-byte boundaries, plus-one
   truncation, combining grapheme, and complete escape handling in Overview
   and Stages.
7. Swift presentation tests cover 292 points, the longest value,
   accessibility size/order, and status refresh without clipping, height/focus/
   identity loss; existing Overview stacks, selection, and filtering remain.
8. Static scans prove there is no feature flag/disable path and no second
   variant lookup table.

The gate fails if any selected Rust or Swift test filter executes zero tests.
It runs through `scripts/test-gate.sh`; no live Codex process or remote UI host
is part of acceptance.

## Rollout

- Update the seven catalog rows and formatter in one implementation change.
- New Runs receive the matrix automatically after deployment.
- Existing Runs continue from frozen snapshots without migration.
- There is no enablement phase, kill switch, or disabled-by-default state.
- Rollback reverts the implementation commit. Runs already frozen with named
  variants remain readable as raw planned values.
- The next ordinary product Run may provide opportunistic visual evidence, but
  no dedicated provider run is required to merge this slice.

## Acceptance checklist

- [ ] The policy fixture is byte-pinned, append-only, and parsed by one strict
      Rust authority with distinct schema and byte-integrity failures.
- [ ] The production catalog contains exactly the seven approved pairs.
- [ ] Typed new-Run admission rejects every matrix mutation with zero Run/work
      writes; old snapshot, legacy fallback, and retrofit paths replay unchanged.
- [ ] Authored `codex_acp` canonicalizes to `codex`; the provider-free production
      bridge preserves full model and one separate effort for all seven rows
      without claiming provider acceptance.
- [ ] Overview and Stages use the same formatter and visibly show friendly
      variant, full ID, effort, and `planned` only from one frozen tuple.
- [ ] No/ambiguous/mismatched assignments and missing effort use distinct
      normative copy; unknown values remain bounded and honest.
- [ ] Existing topology stacks, identities, focus, filtering, authorization,
      retries, fan-out, recovery, and provider lifecycle are unchanged.
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
