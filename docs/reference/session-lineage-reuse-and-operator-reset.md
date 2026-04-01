# Session Lineage Reuse and Operator Reset

Stable reference for the implemented reusable session-lineage slice that was previously tracked by Proposal 018.

## Purpose

The runtime must be able to reuse a provider session when the same logical agent work continues inside one run, while still keeping execution truth, recovery ownership, and operator control fail-closed.

This document is the stable contract for:

- session-lineage ownership and reuse boundaries,
- immutable generation and append-only history semantics,
- binding and invocation-owner compatibility checks,
- budget-driven compaction and invalidation,
- checkpoint-based fresh rehydration,
- operator-triggered per-agent reset from the existing recovery shell,
- and receipt/report surfaces that expose fresh vs reused vs fresh-after-reset truth.

For implementation/proof status, use [../evidence/session-lineage-reuse-and-operator-reset-proof.md](../evidence/session-lineage-reuse-and-operator-reset-proof.md).

## Scope

This reference covers:

- reuse within one run for one logical agent owner,
- opt-in family reuse inside one run,
- persisted session lineage / generation / event truth,
- checkpoint persistence before refresh or reset,
- recovery-shell reset and inspection surfaces,
- and current proof-owning tests for this slice.

It does not introduce:

- cross-run memory,
- cross-agent session sharing,
- provider-routing redesign,
- or any replacement for persisted artifacts and execution truth as the durable source of truth.

## Core Rules

### Reuse is bounded by immutable ownership

Reuse is allowed only when all of these stay compatible:

- same `runID`,
- same `agentID`,
- same `invocationOwnerKey`,
- same binding fingerprint,
- same allowed reuse scope.

The canonical default scope is `same_invocation_owner`.
The only wider scope allowed here is explicit `same_agent_family_within_run`.

`invocationOwnerKey` is the persisted tuple:

- `runID`
- `agentID`
- `stageLineageID`
- `taskName`
- `ownerExecutionLineageID`

This means reuse is not “same agent called again somewhere in the run.”
It is “same logical invocation owner inside the same run,” unless a family-reuse contract explicitly widens it.

### `ownerExecutionLineageID` is imported authority, not session-owned truth

Session reuse reads execution-branch identity from the execution-truth layer documented in [execution-truth-and-recovery.md](execution-truth-and-recovery.md).

Session code may:

- read `ownerExecutionLineageID`,
- persist it inside `invocationOwnerKey`,
- compare it during reuse decisions,
- and surface it in inspection/report output.

Session code may not:

- mint new execution lineage,
- repair execution branch truth,
- or infer branch identity from session history alone.

If trustworthy owner lineage is missing or contradictory, the runtime must fail closed to a fresh session or an `unverifiable_session_history` surface.

### Reuse never replaces durable truth

Even when a provider session is reused:

- artifacts remain canonical,
- receipts remain persisted,
- `AgentExecution` and `StageExecution` remain the durable execution truth,
- reports must remain reconstructable without hidden provider memory.

Session reuse is an execution optimization and continuity aid, not a second truth system.

## Persistence Model

### Lineage, generations, and events have different ownership

The session layer uses three persisted truth shapes:

- `AgentSessionLineage` — stable owner record for one reusable lineage inside one run,
- `AgentSessionGeneration` — immutable generation rows,
- `AgentSessionEvent` — append-only history for resets, invalidations, compactions, and reuse decisions.

The active pointer may move, but historical rows must not be rewritten into a mutable “latest state” record.

### `AgentExecution` persists session provenance

Each `AgentExecution` can persist:

- `sessionLineageID`
- `sessionGenerationID`
- `invocationOwnerKey`
- `sessionReuseScope`
- `sessionFamilyID`
- `sessionReuseDisposition`
- `sessionResetReason`

This is the canonical execution-side record for whether an attempt was:

- `fresh`
- `reused`
- `reused_after_resume`
- `fresh_after_reset`

## Compatibility and Reuse Policy

### Binding compatibility is explicit

Reuse decisions must consider a persisted binding fingerprint rather than a loose “same model” heuristic.

The compatibility surface includes the effective execution contract such as:

- provider,
- model,
- effort,
- workspace and write policy,
- relevant permission context,
- skill/runtime injection context.

If compatibility drifts, reuse must stop and the next invocation must create a fresh generation.

### Family reuse is opt-in only

`same_agent_family_within_run` exists for adjacent same-agent work where the product deliberately wants continuity wider than one invocation owner.

It still requires:

- same run,
- same agent,
- same `sessionFamilyID`,
- compatible binding fingerprint,
- and an explicit opt-in from the workflow/catalog path.

Security-, review-, or audit-style agents should remain on `same_invocation_owner` or `none` unless another implemented slice expands that boundary.

## Budget, Compaction, and Checkpoints

### Reuse is governed by economics, not mere possibility

The runtime may invalidate or compact a lineage when reuse stops paying off.

This decision is owned by measured reuse economics rather than transcript size alone.
Examples of the relevant signals include:

- prompt-fraction growth,
- compaction churn,
- checkpoint frequency,
- savings versus fresh-session baseline,
- and related burn/cost telemetry.

### Fresh rehydration must use checkpoint artifacts

Before budget-driven refresh or operator reset, the runtime may emit a continuation checkpoint artifact.

That checkpoint exists so a fresh generation can rehydrate from durable truth instead of opaque transcript carry-over.
The checkpoint must be persisted only after the primary execution path has validated and persisted the canonical structured outputs it depends on.

## Operator Surfaces

### Reset remains shell-owned

`Reset Agent Session` belongs to the existing recovery spine, not to a parallel settings flow.

The canonical operator surfaces for this slice are:

- the existing recovery coordinator,
- blocked-run and recovery sheet surfaces,
- `AgentSessionInspector`,
- run/report/export surfaces that show session disposition truth.

### Reset must be deterministic

After operator reset:

- the current lineage/generation is retired through append-only history,
- the next invocation for that owner must start fresh,
- and later receipts/reports must show `fresh_after_reset` rather than pretending the lineage continued unchanged.

## Read and Report Order

Readers should prefer:

1. persisted execution-side session provenance on `AgentExecution`,
2. persisted lineage / generation / event records,
3. checkpoint and receipt metadata,
4. UI heuristics only as presentation fallback.

Session history must never override execution truth, and reports must not infer reuse from provider receipts alone when canonical session provenance is already persisted.

## Verification and Proof Owners

The strongest current proof owners for this slice are:

- `AgentSessionTests`
- `GooseAgentExecutorTests`

They cover the implemented claims around:

- same-owner reuse,
- owner/binding compatibility,
- append-only lineage history,
- reset-to-fresh behavior,
- opt-in family reuse,
- budget-driven compaction decisions,
- checkpoint persistence timing,
- and KPI/export surfaces.

For the consolidated proof story, use [../evidence/session-lineage-reuse-and-operator-reset-proof.md](../evidence/session-lineage-reuse-and-operator-reset-proof.md).

## Adjacent References

Use:

- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for canonical execution lineage and recovery truth,
- [runtime-contract.md](runtime-contract.md) for run snapshots and artifact boundaries,
- [provider-binding-truth.md](provider-binding-truth.md) for binding provenance and trust downgrade semantics,
- [operator-experience.md](operator-experience.md) for shell/recovery/report ownership,
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) for contract validation and failed-stage evidence.
