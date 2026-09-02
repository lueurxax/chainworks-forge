# Provider Binding Truth

Stable reference for frozen provider/model truth, Codex planned variants,
provenance, dispatch intent, and run-surface labels.

## Purpose

Run surfaces distinguish the frozen planned binding from mutable settings and
from provider-accepted or actual execution truth. A planned binding records the
compiled request intent; it does not prove that a provider accepted or used the
requested model or effort.

The operator must be able to distinguish:

1. backend-profile intent from the catalog,
2. the planned provider/model/effort frozen at run start,
3. configured, accepted, or actual provider truth when separately available,
4. where each claim came from.

## Scope

This reference covers:

- frozen binding snapshot storage,
- frozen provenance storage,
- the byte-pinned Codex planned-variant policy,
- new-Run admission and historical snapshot compatibility,
- best-effort Codex effort dispatch,
- existing GraphQL readback normalization,
- Overview and Stages planned-label presentation,
- run-surface display rules,
- cross-family mismatch handling,
- historical truth after settings drift.

Related stable docs:

- [provider-platform.md](provider-platform.md)
- [operator-experience.md](operator-experience.md)
- [runtime-contract.md](runtime-contract.md)
- [test-gates.md#codex-planned-variant-slice](test-gates.md#codex-planned-variant-slice)

## Frozen binding rule

The planned provider choice freezes at run start.

The run snapshot stores resolved binding data such as:

- provider family,
- resolved model,
- effort,
- configured provider instance,
- adapter/runtime metadata,
- runtime profile identifier when present,
- resolved adapter family.

Run-centric surfaces must prefer this frozen snapshot over mutable current provider settings.

## Codex planned-variant policy

The canonical policy is
`examples/agents/codex-model-variant-matrix.v1.json`. Version 1 is an
append-only UTF-8 fixture with one final LF, exactly 1,479 bytes, and SHA-256
`b6ad3f2047466a34da42241eae6b790f60bb835d9e6826cb77b51eb3fc558911`.
Vocabulary revisions use a new file and policy ID rather than replacing these
bytes.

The production matrix is:

| Backend profile | Planned model | Effort |
|---|---|---|
| `codex_orchestrator_high` | `gpt-5.6-sol` | `max` |
| `codex_architect_high` | `gpt-5.6-sol` | `xhigh` |
| `codex_audit_high` | `gpt-5.6-sol` | `ultra` |
| `codex_writer_high` | `gpt-5.6-terra` | `high` |
| `codex_builder_high` | `gpt-5.6-terra` | `high` |
| `codex_orchestrator_acp` | `gpt-5.6-terra` | `high` |
| `codex_ops_low` | `gpt-5.6-luna` | `high` |

The historical `codex_ops_low` identifier remains stable. Sol and Terra allow
`low`, `medium`, `high`, `xhigh`, `max`, and `ultra`; Luna allows the same set
except `ultra`. Generic `gpt-5.6`, unknown variants, and Luna `ultra` are not
valid production rows.

`domain::codex_model_variant_policy` owns the strict Rust parser and pinned
loader. It distinguishes the authored catalog provider `codex_acp` from the
canonical resolved/request/readback provider `codex`. The parser rejects
unknown fields, duplicate keys or IDs, undeclared production models,
unsupported efforts, and malformed UTF-8. The pinned loader checks final LF,
length, and digest before parsing.

The same fixture is an explicit app resource.
`CodexModelVariantPolicyLoaderV1` is the only Swift policy authority. Missing,
unreadable, truncated, malformed, digest-mismatched, or undecodable resource
bytes produce unavailable presentation; Swift contains no fallback matrix.

## New-Run admission

`compile_for_new_run_v1` is the typed production entrypoint used by
`StartRun`. It reads workflow and catalog sources once into owned bytes,
rejects duplicate YAML mapping keys before typed decoding, compiles from the
checked values, and validates the exact seven-row Codex matrix before any
Run, Stage, or work insertion transaction opens.

The resulting `NewRunAdmissionV1` contains the compiled plan and snapshot
payloads. The raw catalog snapshot preserves authored provider `codex_acp`;
the resolved plan, work payload, and `ExecutionRequest` use canonical provider
`codex` while preserving model, effort, and backend-profile bytes.

There is no YAML switch, environment override, app preference, rollout flag,
or production bypass. Test-only relaxed fixtures remain behind `#[cfg(test)]`
and are not linked into the daemon.

## Frozen snapshot compatibility

`workflow::snapshot_integrity::verify_complete_pair_v1` is the shared engine
and GraphQL authority for stored workflow/catalog snapshots. It returns only:

- `Absent` when all quartet fields are absent,
- `Verified(pair)` when both UTF-8 JSON payloads and both canonical lowercase
  SHA-256 values are complete and exact,
- typed `Invalid(reason)` for partial, blank, malformed, tampered, mismatched,
  or unparseable state.

Verified historical pairs, including generic and custom model/effort values,
replay byte-for-byte and do not acquire current matrix admission. Snapshot-less
legacy Runs retain their existing mutable live-path recompilation behavior and
receive no planned-label claim. Invalid quartet evidence fails closed to empty
topology and no active-row plan enrichment; stored snapshots never fall back to
live YAML.

## Codex dispatch intent

For policy-admitted rows, the existing production bridge carries the canonical
provider and exact unsuffixed model through the work payload,
`ExecutionRequest`, and `AcpSessionNewSpec` into
`session/new.params.model`. The Codex adapter sends the exact effort once in a
separate best-effort `session/set_config_option` request for
`reasoning_effort`.

Effort requests use exactly one lane:

| Input | Lane | Behavior |
|---|---|---|
| Exact policy model plus exact allowed effort | exact best-effort | send admitted effort bytes once |
| Legacy, generic, custom, combined, case-varied, or unsupported explicit value | legacy fuzzy | preserve the established normalized best-effort behavior |
| No effective effort | none | send no effort configuration request |

`reasoning_effort` never becomes required. Method-not-found or generic
rejection of the best-effort request does not suppress the prompt and does not
create an accepted/configured/actual claim. Provider fallback, silent remote
defaults, and later provider-side changes do not rewrite the frozen planned
pair.

## Existing readback contract

Planned-variant readback uses the existing GraphQL schema and defines no new
root, field, document, identifier, or authorization path.

- `runStageTopology` compiles only a verified frozen snapshot pair.
- The plan-enrichment part of `activeAgentExecutions` uses the same verifier.
- Overview selects exactly one frozen occurrence whose canonical provider and
  model match the active execution and takes provider/model/effort together
  from that occurrence.
- Missing, ambiguous, owner-only, dynamic-unrepresented, or mismatched
  occurrences render unavailable rather than combining fields from different
  sources.
- Health fallback or retry may show planned identity only when the unique
  frozen occurrence still matches the effective binding.
- Resolver-local normalization emits `codex` only for registered Codex aliases
  in active execution readback. Stored rows, snapshot bytes, stage-scoped
  `agentExecutions.provider`, non-Codex aliases, and unknown providers remain
  byte-identical.
- The Overview current-stage filter hides unresolved, completed,
  mapped-noncurrent, and stale rows and never reassigns them to another stage.

The existing presentation does not claim distinct row identity for concurrent
same-agent executions.

## Planned-label presentation

Overview and Stages use one Swift formatter. Policy-known values render:

```text
Sol · gpt-5.6-sol · max · planned
Terra · gpt-5.6-terra · high · planned
Luna · gpt-5.6-luna · high · planned
```

The full help and accessibility value includes provider context, for example
`Codex · Sol · gpt-5.6-sol · max · planned`.

Recognized generic or missing-effort states remain explicit:

```text
Codex · gpt-5.6 · high · planned
Codex · Sol · gpt-5.6-sol · Planned effort not recorded
Planned assignment unavailable
```

Unknown model or effort text is never interpolated into planned copy. Non-Codex
copy remains unchanged. For regular Dynamic Type sizes the complete planned
assembly is visible. At accessibility sizes the separate Sol/Terra/Luna token
remains visible while only the suffix may truncate; Help and VoiceOver retain
the complete value. Each represented Stage occurrence exposes the planned
value exactly once in the accessibility tree.

The implementation preserves existing topology geometry, selection, keyboard
focus, status-only refresh identity, and provider lifecycle. It adds no button,
menu, shortcut, capsule, badge, feature flag, or disable path.

## Frozen provenance rule

The run also stores per-agent provenance for how the resolved model was chosen.

Supported provenance states:

- `backendProfileDefault`
- `configuredProviderDefault`
- `runOverride`
- `unverifiable`

Historical explanation must come from frozen provenance, not from reverse-engineering current settings later.

## Coherence policy

Cross-family model/provider mismatches must not silently normalize into ordinary truth.

Rules:

- start/preflight validate family/model coherence where possible,
- unusual bindings are surfaced explicitly as warnings,
- run-centric surfaces show that the binding is unusual instead of flattening it into a neutral label.

Example of what must not happen:

- showing `claude_acp · gpt-5-codex` as if it were an ordinary expected binding without warning or provenance.

## Run-surface expectations

Run-centric surfaces present these facts:

- frozen planned provider family,
- frozen planned model and effort,
- the `planned` qualifier when only compiled request intent is known,
- runtime profile / adapter family when they materially differ from the catalog default execution lane,
- provenance source,
- mismatch warning state when applicable.

This applies to:

- idea detail / run detail,
- workflow map side panels,
- run comparison,
- immutable run reports,
- latest run summary.

## Historical truth after drift

Current machine settings may change after a run completes.

Therefore:

- historical run surfaces must not recompute origin from current provider settings,
- the frozen snapshot remains authoritative,
- `unverifiable` is valid and preferable to invented certainty.

## Verification

`./scripts/test-gate.sh codex-planned-variant-slice` is the retained stable
proof gate. It is provider-free and covers the pinned policy, exact-seven
new-Run admission, snapshot compatibility, production ACP bridge, closed
effort lanes, GraphQL normalization and topology, shared Swift formatting,
resource failures, geometry, Dynamic Type, accessibility, selection, and focus.
Every selected Rust and Swift filter must execute at least one test.

## Cross-doc contract

This document extends the stable provider baseline in
[provider-platform.md](provider-platform.md) with planned-binding admission,
historical truth, dispatch-intent, and presentation requirements.

It also feeds:

- [operator-experience.md](operator-experience.md) for reports and comparison,
- [run-control.md](run-control.md) because cancelled/failed history must remain truthful,
- [provider-platform.md](provider-platform.md) because remediation should improve provider truth rather than hide it.
