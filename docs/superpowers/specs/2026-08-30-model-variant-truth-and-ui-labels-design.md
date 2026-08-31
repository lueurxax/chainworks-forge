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
- `activeAgentExecutions` already exposes execution model and identity.
- Overview already has the selected Run's topology map, so it can obtain the
  frozen effort for an active execution without a new GraphQL field.
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

The fixture is 1,446 bytes and has SHA-256
`b9119b3e4375d46d8e9ad29b615ca3d8385357be8f7415869e4c51590bc31395`.
Version 1 is append-only. A later vocabulary adds a new file and policy ID
without replacing these bytes.

`domain::codex_model_variant_policy` is the dependency-leaf parser and
registry. It rejects duplicate JSON keys, unknown fields, duplicate model or
profile IDs, an undeclared production model, an unsupported effort, malformed
UTF-8, missing final LF, wrong byte length, or wrong digest. Workflow, ACP
tests, and Swift formatter tests import generated/typed values from this single
policy source rather than maintaining another matrix.

The Xcode bundle contains the same retained fixture for validation and
presentation lookup. Rust remains the run-creation authority; Swift validation
is feedback only.

### 2. Fresh catalog validation

The Rust workflow compiler validates every newly authored catalog before Run
creation:

- every Codex profile uses a declared model and allowed effort;
- each of the seven reserved production profile IDs, when present,
  byte-matches its policy row and uses provider `codex_acp`;
- generic `gpt-5.6`, unknown variants, and Luna `ultra` reject; and
- duplicate YAML mapping keys reject before typed decoding.

The focused source gate separately requires canonical
`examples/agents/agents.yaml` to contain all seven reserved rows exactly once
and no additional Codex profile. Bounded test catalogs may use other profile
IDs with supported pairs; they cannot impersonate or weaken a reserved row.

The compiler then writes the unchanged authored provider, model, effort, and
backend profile into the existing frozen catalog and resolved-agent fields.
No new Run schema version, DB column, or execution contract is needed.

The canonical production-catalog check applies only to new Run compilation.
`compile_from_snapshot_json` continues to decode previously frozen catalogs
without applying the current production matrix. Old generic or custom Runs
therefore resume byte-for-byte with their existing behavior.

There is no YAML switch, environment override, app preference, rollout flag,
or test-only production bypass. Non-production tests may construct a bounded
fixture catalog explicitly through test helpers; those helpers are not linked
into the daemon binary.

### 3. Planned dispatch

For a new Run, every existing InvokeAgent producer continues through its
current owner, queue, retry, fan-out, and recovery path. This proposal adds no
producer branch.

The existing execution request receives the full frozen model and effort as
separate values. The Codex adapter must not collapse them into
`model/effort`, replace a named variant with generic `gpt-5.6`, or apply
case folding to a policy-known value. Adapter unit tests cover all seven
production rows.

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
- Overview keeps using `activeAgentExecutions` for active execution identity
  and the existing topology map for stage context. It obtains effort from the
  matching frozen topology occurrence for the same stage and agent.
- A frozen agent ID has one backend profile, so repeated tasks for that agent
  share the same planned model/effort. If no matching occurrence is available,
  Overview renders the execution model and `effort unavailable`.

The change must not alter root queries, authorization, topology stacks,
filtering, occurrence identity, focus, selection, pagination, or stale-response
handling. It must not add a second query or broaden data access.

### 5. Shared presentation

One pure Swift formatter receives provider, frozen model, and frozen effort.
For known Codex IDs it emits:

```text
Codex · Sol · gpt-5.6-sol · max · planned
Codex · Terra · gpt-5.6-terra · high · planned
Codex · Luna · gpt-5.6-luna · high · planned
```

Unknown or legacy values remain honest:

```text
Codex · gpt-5.6 · high · planned
Codex · unknown-model · effort unavailable · planned
```

Rules:

- friendly names are byte-exact lookup labels, never proof of provider use;
- the full model ID, effort, and `planned` qualifier remain visible in both
  Overview and Stages;
- the compact row may wrap but must not overlap, hide the variant, or change
  card dimensions while status updates;
- existing help/accessibility text uses the same complete formatter output;
- no new button, menu, shortcut, selection behavior, or topology ownership is
  introduced;
- missing effort renders `effort unavailable`;
- unknown nonempty values are trimmed and capped to 96 UTF-8 bytes with
  `...[truncated]`; control characters render as escaped code points; and
- non-Codex providers retain current copy.

Overview and Stages must call the same formatter. Source scans reject local
Sol/Terra/Luna switch statements elsewhere in the app.

## Failure behavior

| Condition | Result |
|---|---|
| New production catalog differs from the policy | compilation fails before Run creation |
| Policy fixture missing, malformed, or digest-mismatched | compilation fails; Xcode validation reports the same issue |
| Old frozen Run contains generic or custom model | replay unchanged; raw planned value is shown |
| Overview cannot correlate topology effort | model remains visible; `effort unavailable` |
| Provider ignores or later changes the request | existing runtime behavior; planned UI remains planned |
| Unknown model/effort reaches formatter | bounded escaped raw value, never a friendly false match |

No new retry, fallback, cancellation, failure-kind, operator-action, or
quarantine behavior is introduced.

## Verification gate

Add focused gate `codex-planned-variant-slice`. It is provider-free and runs
only the following:

1. Policy tests recompute the exact 1,446-byte digest, parse strict JSON, and
   reject duplicate keys, unknown fields, malformed rows, unsupported effort,
   generic production model, Luna `ultra`, and in-place v1 mutation.
2. Workflow tests compile the canonical seven rows and mutation-test every
   model, effort, provider, duplicate, missing profile, and extra Codex profile.
3. Frozen replay tests prove old generic/custom snapshots remain byte-identical
   and do not receive current production validation.
4. Adapter unit tests prove each production pair reaches the existing request
   as separate full model and effort values, with no generic substitution or
   combined `model/effort` encoding.
5. Existing GraphQL tests prove topology still returns frozen provider, model,
   and effort and active execution identity is unchanged. No new document or
   schema snapshot is added.
6. Swift unit tests prove one formatter produces the normative Sol, Terra,
   Luna, generic, missing-effort, unknown, control-character, and truncation
   outputs for both Overview and Stages.
7. Swift presentation tests cover the current compact card width and
   accessibility value, and prove existing Overview topology stacks, focus,
   selection, and stage filtering remain present.
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
      Rust authority.
- [ ] The production catalog contains exactly the seven approved pairs.
- [ ] Fresh compilation rejects every matrix mutation before Run creation.
- [ ] Old frozen Runs replay unchanged.
- [ ] Adapter tests preserve full model and effort separately for all seven
      profiles without claiming provider acceptance.
- [ ] Overview and Stages use the same formatter and visibly show friendly
      variant, full ID, effort, and `planned`.
- [ ] Missing and unknown values remain bounded and honest.
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
