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
6. Preserve every pre-change frozen run byte-for-byte and label its generic
   identity as legacy/unverified rather than inferring a variant.
7. Ship the behavior enabled by default with no feature flag or disable path.

## Non-goals

- Persist or display `accepted_model`, `accepted_effort`, option-snapshot
  revisions, prompt permits, or provider-acceptance receipts.
- Reuse an exact-pair Codex physical session across separate invocations.
- Change provider-session resurrection, output-only recovery, P079 repair
  materialization, provider fallback, or escalation policy.
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

### 1. Fresh catalog validation

The Rust workflow compiler and the Swift catalog compiler use one checked-in
matrix fixture containing the seven rows above. For every fresh catalog, a
Codex profile must use an exact supported variant and a valid effort; when a
profile uses one of the seven production IDs, its pair must match the fixture.
The canonical production-catalog gate additionally requires all seven rows and
no additional Codex profile. Fresh compilation fails before run creation when
any of these conditions is true:

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
frozen catalog and resolved-agent fields. This slice does not introduce a new
snapshot schema solely to restate those values.

### 2. Frozen replay boundary

Runs created before this change retain their workflow/catalog snapshot bytes
and current adapter behavior. Generic `gpt-5.6` in an existing frozen snapshot
is treated as `legacy_generic_v0`; it is not rejected on resume and is never
mapped to Sol, Terra, or Luna.

Exact-pair enforcement is selected only when all three frozen values match:

```text
invocation creates a normal stage-scoped AgentExecution
provider == codex_acp
model in {gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna}
effort is present and valid for that model
```

The normal stage-scoped class includes its ordinary retry and provider-fallback
attempts when the selected frozen binding is Codex. P079 repair turns, P086
attach/resurrection turns, Steward turns, and other non-stage owners retain
their current adapter behavior and cannot emit an accepted/configured claim.
Anything else follows the pre-change legacy path. Fresh compilation prevents
new generic or invalid Codex snapshots, while frozen replay remains compatible.

### 3. Exact ACP configuration

For an exact-pair invocation the adapter performs this sequence before writing
any `session/prompt` bytes:

1. Launch a fresh Codex ACP process and send `session/new` with the exact model
   ID from the frozen binding.
2. Read the returned `configOptions` as the first working snapshot.
3. Resolve the `model` option by one case-insensitive full-value match. Alias,
   substring, token, display-name guessing, and fallback to an unadvertised raw
   value are forbidden.
4. Send required `session/set_config_option(model, exact_model)`.
5. Require a successful response with `configOptions` whose model
   `currentValue` equals the exact requested model. That response replaces the
   working snapshot.
6. Resolve `reasoning_effort` from the updated working snapshot using the same
   exact-value rule.
7. Send required
   `session/set_config_option(reasoning_effort, exact_effort)`.
8. Require a successful response with `configOptions` proving both final
   `model.currentValue` and `reasoning_effort.currentValue` equal the requested
   pair.
9. Apply every configuration update already observed by the handshake read
   loop to the same bounded in-memory option snapshot in wire order. A missing,
   malformed, duplicate, or contradictory model/effort update rejects the
   invocation.
10. Reverify the current in-memory pair immediately before the prompt write.
    Only then send the first prompt.

Every response is consumed in wire order. A success response without the
required bounded `configOptions` is not proof. Missing/duplicate options,
ambiguous values, malformed responses, send failure, provider rejection, or a
final mismatch closes the child and returns
`ACP_CODEX_EXACT_CONFIGURATION_REJECTED`; prompt-send count remains zero.

The in-memory option snapshot is invocation-local and discarded with the child;
it is not a durable acceptance receipt. Updates received after prompt dispatch
cannot create an accepted/configured UI claim because this slice exposes only
planned truth. The required path does not log option values, session
identifiers, or raw provider payloads. Existing bounded error/redaction
behavior remains in force.

### 4. Session lifetime

An exact-pair normal stage invocation owns one fresh physical Codex session. It is not
eligible for cross-invocation live-session reuse or P086 resurrection in this
slice. Retry creates another fresh session and repeats the complete required
configuration sequence.

This deliberate performance tradeoff avoids treating process-local or
historical evidence as durable acceptance. Efficient reuse is deferred to the
provider-acceptance child document and cannot be enabled by a flag.

Legacy generic frozen invocations retain their current reuse behavior.

### 5. Readback

`runStageTopology.occurrences` remains the Stages source and continues to
project model and effort from the frozen run plan.

`GqlAgentExecution` gains additive nullable field `effort`. The resolver derives
it from the execution's `backend_profile_id` in the run's frozen catalog. It
does not read the current catalog and does not add or backfill a database
column. Missing snapshot/profile/binding yields `null`; it never guesses.

The macOS `P031ActiveAgentExecutionReadModel` gains nullable `effort` and the
existing run-detail document selects it. Older daemons omit the field and
decode as `nil`; the current schema-capability boundary still controls daemon
compatibility.

No MCP, report, artifact, receipt, or runtime-health schema changes in this
slice.

### 6. Shared presentation

One pure formatter owns model/effort copy for Overview active-agent rows and
Stages occurrence rows. It receives only provider, planned model, planned
effort, and legacy state.

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

- `Sol`, `Terra`, and `Luna` are friendly labels only for the three exact IDs.
- The exact model ID remains visible; compact copy may truncate visually but
  accessibility and copy text contain the complete formatter output.
- `planned` is mandatory. This slice never renders `accepted`, `configured`,
  `actual`, or equivalent claims.
- Missing effort renders `effort unavailable`; unknown nonempty values render
  bounded escaped text and never map to a known effort.
- Non-Codex providers retain their existing requested-identity copy.
- Status, stage/task labels, model copy, and effort copy remain separate
  accessibility values; the formatter does not change selection or focus.

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
| Legacy generic frozen run | Existing legacy path; UI says unverified | Existing behavior |
| Active readback cannot resolve frozen profile | `effort = null`; UI says unavailable | No mutation |

No failure falls back to a generic model, provider default, weaker effort, or
another provider. Existing workflow retry/escalation may react to the terminal
typed failure, but this slice does not alter that policy.

## Verification gate

Add focused gate `codex-model-variant-slice`. It is provider-free and runs:

1. Rust and Swift matrix parity against one checked-in fixture.
2. Fresh compiler positives for every approved production row and bounded
   non-production fixture catalogs using supported exact pairs.
3. Mutation negatives for every production profile, generic model, unknown
   model/effort, Luna `ultra`, duplicate profile, missing canonical production
   profile, and undeclared extra production Codex profile.
4. Frozen legacy replay proving snapshot bytes are unchanged and generic model
   remains admissible only through the legacy path.
5. Fake ACP success proving ordered model then effort configuration and exactly
   one prompt after both exact `currentValue` checks.
6. Fake ACP negatives for alias-only, substring-only, duplicate, missing,
   malformed, stale-snapshot, rejected, and mismatched responses; each asserts
   zero prompt bytes and bounded child cleanup.
7. Retry proof that a second attempt launches a fresh session and repeats both
   required checks.
8. GraphQL resolver tests proving effort comes from the frozen profile, not the
   current catalog, and missing/ambiguous bindings return null.
9. Swift decoding and formatter goldens for Sol, Terra, Luna, legacy generic,
   missing effort, unknown bounded values, compact copy, full copy, and
   accessibility output.
10. Hosted Overview and Stages tests proving both surfaces use byte-identical
    formatter output and no accepted/configured wording appears.
11. Structural scans proving there is no feature flag, environment bypass,
    current-catalog read in run readback, or second formatter.

The gate fails when either selected Swift suite executes zero tests. It does
not invoke a live provider, network, remote UI host, or another proposal gate.

## Rollout

- The approved matrix and exact ACP sequence become default behavior for every
  newly compiled run after release.
- There is no disable switch, experiment percentage, or operator opt-in.
- Pre-change frozen runs continue unchanged and visibly say legacy/unverified.
- A configuration rejection fails that attempt visibly and lets existing retry
  policy decide the next action; it never silently weakens the pair.
- Operational observation from a normal later run is useful but not required
  to merge this provider-free slice.

## Acceptance checklist

- [ ] The active catalog contains exactly the approved seven Codex pairs.
- [ ] Rust and Swift fresh compilers reject every matrix mutation before run
      creation.
- [ ] Existing generic frozen runs replay without byte mutation and without a
      fabricated Sol/Terra/Luna label.
- [ ] Exact invocations verify model and effort in order before the first
      prompt, and every negative fixture proves zero prompt bytes.
- [ ] Exact invocations use a fresh physical session; retry repeats the full
      sequence.
- [ ] Overview and Stages show the same complete friendly variant, exact model
      ID, effort, and `planned` qualifier.
- [ ] Active-agent effort is derived only from the frozen backend profile and
      remains nullable when unavailable.
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
