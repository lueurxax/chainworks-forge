# Context Strategy and Experiment Framework Proof

Current implementation and proof status for the context-strategy, experiment, lazy-evidence, and strategy-recommendation slice consolidated from Proposal 019.

## Status

| Field | Value |
|---|---|
| Slice | Context Strategy and Experiment Framework |
| Source contract | [../reference/context-strategy-and-experiment-framework.md](../reference/context-strategy-and-experiment-framework.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary proof owners | `Proposal019Tests`, `RuntimeSessionBridgeTests`, `RuntimeAgentExecutorTests`, `OrchestratorTests`, canonical `proposal-019` gate |
| Last consolidated documentation refresh | `2026-04-01` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- the app can run the same workflow under multiple named context-strategy profiles,
- selected strategy truth is frozen into immutable run state and survives resume/clone boundaries correctly,
- `HandoffCompiler` feeds the existing `ExecutionPacket` owner rather than replacing it,
- lazy evidence is retrieved through a real executable `get_lazy_artifact` helper rather than descriptive-only metadata,
- strategy telemetry is persisted through the existing KPI lane with normalized signals including lazy-evidence hit count and hit rate,
- retryable non-contract failures can escalate model tier without erasing canonical contract-failure evidence,
- strategy scoring and recommendation logic use normalized telemetry rather than only coarse duration/cost totals,
- and strategy badges plus recommendation output remain shell-owned and reproducible.

## Accepted current-head proof owners

The strongest current-head proof owners are:

- `Proposal019Tests`
- `RuntimeSessionBridgeTests`
- `RuntimeAgentExecutorTests`
- `OrchestratorTests`
- `scripts/test-gate.sh proposal-019`

The accepted current-head gate for this slice is:

```bash
./scripts/test-gate.sh proposal-019
```

Fresh same-head proof recorded during final implementation closure:

- focused same-head slice passed `63` tests in `4` suites
- canonical gate `proposal-019` passed `63/63`
- result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-019-20260401-182256.xcresult`

## Requirement coverage

The final implementation closure established proof for:

- three-or-more named profiles on the same workflow,
- frozen strategy truth in run snapshots,
- strategy handoff compilation through the existing packet owner,
- executable lazy-evidence retrieval,
- normalized KPI/export telemetry for strategy comparison,
- escalation only for retryable non-contract failures,
- shell-owned badges and comparison/report recommendations,
- measurable selective-compression savings,
- and recommendation degradation to `Inconclusive` / `Insufficient evidence` when proof is incomplete.

## Canonical proof lane

The canonical proof owner for this slice is singular:

- focused current-head macOS test suites for strategy runtime seams
- `scripts/test-gate.sh proposal-019`

The gate is canonical because it is the named repository-owned path for this slice and not just an ad-hoc focused `xcodebuild test` invocation.

## Consolidation note

The old Proposal 019 draft, review, evidence pack, research pack, and implementation audits were implementation-trail artifacts.

They have been superseded by:

- [../reference/context-strategy-and-experiment-framework.md](../reference/context-strategy-and-experiment-framework.md)
- this proof document

This slice should now be treated as stable implemented behavior rather than active proposal work.

## Remaining caution

There is no blocking conformance gap in the current proof story.

The practical caution is ordinary current-head drift:

- strategy telemetry semantics,
- provider cache-effectiveness normalization,
- and shell-owned recommendation wording

should be rechecked on future heads through the same canonical `proposal-019` gate instead of being assumed stable forever.

## Recommended usage

Use:

- [../reference/context-strategy-and-experiment-framework.md](../reference/context-strategy-and-experiment-framework.md) for the stable contract,
- [../reference/runtime-contract.md](../reference/runtime-contract.md) for frozen run-start truth,
- [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) for contract failure and narrow recovery,
- [../reference/session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md) for continuity and checkpoint semantics,
- [../reference/test-gates.md](../reference/test-gates.md) for the canonical verification lane.
