# Provider Binding Truth

Stable reference for frozen provider/model truth, provenance, and cross-family warning behavior that were previously tracked by Proposal 011.

## Purpose

Run surfaces must describe the binding the runtime actually used, not a convenient shorthand assembled later from mutable settings.

The operator must be able to distinguish:

1. backend-profile intent from the catalog,
2. configured provider selected at run start,
3. resolved runtime provider family and model,
4. where that model came from.

## Scope

This reference covers:

- frozen binding snapshot storage,
- frozen provenance storage,
- run-surface display rules,
- cross-family mismatch handling,
- historical truth after settings drift.

Related stable docs:

- [provider-platform.md](provider-platform.md)
- [operator-experience.md](operator-experience.md)
- [runtime-contract.md](runtime-contract.md)

## Frozen binding rule

Provider choice freezes at run start.

The run snapshot stores resolved binding data such as:

- provider family,
- resolved model,
- effort,
- configured provider instance,
- adapter/runtime metadata.

Run-centric surfaces must prefer this frozen snapshot over mutable current provider settings.

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

- showing `claude_code · gpt-5-codex` as if it were an ordinary expected binding without warning or provenance.

## Run-surface expectations

Run-centric surfaces should make these facts legible:

- resolved provider family,
- resolved model,
- effort,
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

## Cross-doc contract

This document extends the stable provider baseline in [provider-platform.md](provider-platform.md) with stricter historical truth requirements.

It also feeds:

- [operator-experience.md](operator-experience.md) for reports and comparison,
- [run-control.md](run-control.md) because cancelled/failed history must remain truthful,
- [goose-provider-remediation.md](goose-provider-remediation.md) because remediation should improve provider truth rather than hide it.
