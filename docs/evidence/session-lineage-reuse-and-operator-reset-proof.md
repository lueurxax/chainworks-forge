# Session Lineage Reuse and Operator Reset Proof

Current implementation and proof status for the reusable session-lineage slice consolidated from Proposal 018.

## Status

| Field | Value |
|---|---|
| Slice | Session Lineage Reuse and Operator Reset |
| Source contract | [../reference/session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary evidence owner | focused current-head session-reuse suites in `Chainworks ForgeTests` |
| Last consolidated documentation refresh | `2026-04-01` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- the same logical invocation owner inside one run can reuse a compatible provider session instead of always cold-starting,
- reuse is bounded by persisted owner and binding truth rather than loose agent identity alone,
- lineage history is stored as stable owner records, immutable generations, and append-only events,
- operator reset from the existing recovery shell forces the next invocation to start fresh,
- opt-in family reuse exists without reopening cross-run or cross-agent memory,
- budget guard decisions are driven by measured reuse economics,
- continuation checkpoints are persisted after the execution path has already persisted validated structured-output truth,
- receipts and report/export surfaces expose reuse/fresh/reset truth explicitly.

## Accepted current-head proof owners

The strongest current-head proof owners are:

- `AgentSessionTests`
- `RuntimeAgentExecutorTests`

The focused accepted proof lane on the current tree is:

- direct macOS `xcodebuild build`
- focused `xcodebuild test` slice for `AgentSessionTests` and `RuntimeAgentExecutorTests`

Fresh same-head proof recorded in the final implementation audit:

- build succeeded on current head
- focused `proposal-018` slice passed `26` tests in `2` suites
- result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-018-r10-test-20260401-080947.xcresult`

## Requirement coverage

The final implementation audit closed all in-scope requirements for this slice, including:

- same-owner same-run reuse,
- owner/binding compatibility checks,
- stable lineage plus immutable generations and append-only events,
- shell-owned per-agent reset,
- deterministic fresh-next-invocation behavior after reset,
- report/export visibility for reuse disposition,
- retry/resume reuse within safe lineage,
- opt-in family reuse within one run,
- measured budget/compaction policy,
- continuation-safe checkpoint persistence,
- fresh lineage on clone-run boundaries,
- persisted session provenance on `AgentExecution`,
- KPI/export visibility for reuse savings.

## Consolidation note

The old Proposal 018 draft, reviews, implementation audits, evidence packs, and proposal-local research files were implementation-trail artifacts.

They have been superseded by:

- [../reference/session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md)
- this proof document

This slice should now be treated as stable implemented behavior, not as an active proposal dependency.

## Remaining caution

The remaining caution is proof packaging, not contract completeness:

- the final accepted evidence was a focused direct current-head lane,
- not a dedicated long-lived wrapper gate with a permanent named top-level command.

That does not reopen the slice.
It only means later heads should reproved this behavior through the same focused build/test lane instead of inheriting the proof by assumption.

## Recommended usage

Use:

- [../reference/session-lineage-reuse-and-operator-reset.md](../reference/session-lineage-reuse-and-operator-reset.md) for the stable contract,
- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) for execution lineage and recovery authority,
- [../reference/provider-binding-truth.md](../reference/provider-binding-truth.md) for binding provenance,
- [../reference/operator-experience.md](../reference/operator-experience.md) for recovery-shell ownership.
