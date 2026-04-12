# Execution Truth and Recovery Proof

Current implementation and proof status for the execution-truth and recovery slice consolidated from Proposal 016.

## Status

| Field | Value |
|---|---|
| Slice | Execution Truth and Recovery |
| Source contract | [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary evidence owner | current-head unit/runtime suites in `Chainworks ForgeTests` |
| Last consolidated documentation refresh | `2026-03-30` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- neutral finish markers do not silently count as success,
- provider/app limit exhaustion after output preserves durable output truth and records a canonical non-success outcome,
- `WorkflowOrchestrator` persists canonical outcome, transport metadata, runtime provider/model, and outcome envelopes onto `AgentExecution`,
- output validation after durable output settles as `failed_after_output_validation`,
- interrupted approval flows restore operator-visible approval context instead of silently re-executing the stage,
- recovery and report builders read stage-owned failure/recovery evidence instead of relying only on heuristic artifact scans.

## Accepted current-head proof owners

The strongest current-head proof owners are:

- `RuntimeAgentExecutorTests`
- `OrchestratorTests`
- `ResumeManagerTests`
- `RecoveryCoordinatorTests`
- `Proposal013Tests`

Important owner examples on the current tree:

- `Neutral finish marker alone does not count as success`
- `Limit exhaustion after output preserves artifacts and records canonical outcome`
- `Orchestrator` coverage that persists `canonicalOutcome`, `providerStopReason`, `runtimeProvider`, `runtimeModel`, and `outcomeEnvelopeJSON`
- approval-resume coverage in `ResumeManagerTests`
- report/recovery synthesis coverage in `Proposal013Tests`

## Consolidation note

The old Proposal 016 draft, review, evidence-pack, and proposal-local research files were implementation-trail artifacts.

They have been superseded by:

- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md)
- this proof document

This slice should now be treated as stable reference behavior, not as an active proposal dependency.

## Remaining caution

The remaining caution is about proof packaging rather than the contract itself:

- the current repository does not expose a dedicated `proposal-016` wrapper gate,
- proof therefore lives in targeted runtime suites rather than one named top-level gate,
- later heads should still be reproved instead of inheriting this documentation by assumption.

That caution does not reopen the slice.
It only means the evidence story is suite-based rather than wrapper-gate-based.

## Recommended usage

Use:

- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) for the stable contract,
- [../reference/runtime-contract.md](../reference/runtime-contract.md) for adjacent snapshot and artifact rules,
- [../reference/provider-binding-truth.md](../reference/provider-binding-truth.md) for binding provenance and downgrade semantics,
- [../reference/run-control.md](../reference/run-control.md) for the cancellation-control layer.
