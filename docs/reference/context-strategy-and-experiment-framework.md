# Context Strategy and Experiment Framework

Stable reference for the implemented context-strategy, experiment, lazy-evidence, and strategy-recommendation slice that was previously tracked by Proposal 019.

## Purpose

The runtime must be able to run the same workflow under different named context strategies, measure the result through one canonical telemetry lane, and compare those strategies through existing shell-owned report and comparison surfaces.

This document is the stable contract for:

- strategy-profile selection and run-start freezing,
- strategy-aware handoff compilation,
- lazy evidence retrieval through an executable helper path,
- model-tier escalation for retryable non-contract failures,
- normalized strategy telemetry and scoring,
- and shell-owned badges, recommendation output, and operator overrides.

For implementation and proof status, use [../evidence/context-strategy-and-experiment-framework-proof.md](../evidence/context-strategy-and-experiment-framework-proof.md).

## Scope

This reference covers:

- multiple named `ContextStrategyProfile`s,
- run-level manual override and deterministic cohort assignment,
- handoff shaping through `HandoffCompiler`,
- `selective_compression_and_escalation`, `manual_like_long_continuity`, `fresh_control`, and the current baseline profile,
- lazy artifact retrieval via `get_lazy_artifact`,
- normalized strategy telemetry persisted through the existing run/session KPI lane,
- recommendation output and strategy badges on current shell-owned surfaces,
- and the canonical `proposal-019` gate.

It does not introduce:

- a second live configuration authority,
- a second execution-packet owner,
- a second metrics-truth lane,
- a separate experiment console,
- or blind auto-reruns that hide canonical contract failure evidence.

## Core Rules

### Strategy truth is frozen at run start

`steward.yml` is only a compile/start-time source.

The selected strategy profile must be normalized and frozen into the immutable run snapshot alongside the existing workflow, catalog, and runtime settings.

Resume, retry, clone-from-frozen, and historical comparison read that frozen strategy truth from the run snapshot rather than from live disk config.

Clone-with-current may recompile against current disk config, but that must create a new run identity.

### `HandoffCompiler` is upstream of the canonical packet owner

`HandoffCompiler` owns context selection, summarization, promoted-artifact inclusion, and lazy references.

`GooseSessionBridge` remains the only owner of the final provider-facing `ExecutionPacket`.

`BindingFingerprintBuilder` continues to hash the effective execution surface that is actually sent to the provider, including strategy-owned handoff material once it has been embedded into the final packet.

### Lazy evidence is on-demand, not prompt bloat

Strategies may keep non-essential artifacts out of the initial prompt.

When that happens, the runtime exposes a real executable `get_lazy_artifact` helper plus a manifest and canonical artifact pointers.

This helper is an extension of the existing execution-packet/file-attachment seam rather than a second packet or tool authority.

### Escalation is narrow and does not erase canonical failure evidence

`selective_compression_and_escalation` may retry at a higher model tier only for explicitly retryable non-contract failure classes.

Contract mismatch and output-validation failure remain canonical failed-stage evidence and stay visible through narrow recovery and report/export surfaces.

### Telemetry extends the existing KPI lane

Strategy telemetry is not stored in a free-floating `strategyMetricsJSON` blob.

It extends the existing run/session KPI and lineage-report lane with normalized strategy signals such as:

- payload reduction,
- cache effectiveness,
- lazy-evidence hit count and hit rate,
- compaction churn,
- escalation counts by retryable failure class,
- and operator-promoted handoff burden.

### Recommendation output remains shell-owned and reproducible

`StrategyBadge`, comparison output, and recommendation surfaces extend the current run/report/comparison/recovery shell spine.

Recommendation output is trustworthy only when emitted by that shell-owned lane and when it cites:

- proof owner,
- evaluation set,
- hold criteria,
- and recommendation state.

If canonical telemetry is incomplete, the output must degrade to `inconclusive` or `insufficient_evidence`.

## Strategy Profiles

The implemented strategy layer supports named profiles resolved from steward configuration and normalized into runtime types:

- `current_mixed_baseline`
- `manual_like_long_continuity`
- `selective_compression_and_escalation`
- `fresh_control`

Profile resolution supports:

- explicit operator selection,
- deterministic cohort assignment for experiments,
- frozen persistence on `Run`,
- and shell-visible recommendation state.

## Persistence and Runtime Surfaces

### `Run` persists strategy truth

The `Run` model persists:

- `contextStrategyProfileID`
- `strategyAssignmentMode`
- `strategyRecommendationState`
- frozen run-start snapshot fields that round-trip strategy truth through resume and clone

### `AgentExecution` persists strategy telemetry

The `AgentExecution` model persists strategy and limit-pressure data such as:

- `inputPayloadBytes`
- `handoffMode`
- `limitPressureSignalsJSON`
- `modelTierUsed`

These execution-side records feed the canonical KPI export and recommendation logic.

### Strategy reports read canonical telemetry first

Strategy scoring and recommendation logic read the normalized KPI lane and shell-owned report/comparison data before using coarse totals.

Important normalized signals include:

- cache-effectiveness,
- lazy-evidence hit rate,
- payload reduction,
- compaction churn,
- escalation burden,
- and operator override count.

## Operator Surfaces

The canonical operator-owned surfaces for this slice are:

- run start / idea launch strategy selection,
- `StrategyBadge` on run, report, comparison, and recovery surfaces,
- report and comparison recommendation output,
- and operator-promoted handoff artifacts inside current report/recovery ownership.

No separate experiment dashboard is required for this slice to remain valid.

## Verification Owners

The strongest current proof owners for this slice are:

- `Proposal019Tests`
- `GooseSessionBridgeTests`
- `GooseAgentExecutorTests`
- `OrchestratorTests`
- `scripts/test-gate.sh proposal-019`

For the consolidated proof story, use [../evidence/context-strategy-and-experiment-framework-proof.md](../evidence/context-strategy-and-experiment-framework-proof.md).

## Adjacent References

Use:

- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for execution outcome and recovery authority,
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) for contract-failure evidence and narrow recovery,
- [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md) for continuity and checkpoint economics,
- [operator-experience.md](operator-experience.md) for shell ownership,
- [runtime-contract.md](runtime-contract.md) for frozen snapshot rules,
- [test-gates.md](test-gates.md) for the canonical `proposal-019` proof lane.
