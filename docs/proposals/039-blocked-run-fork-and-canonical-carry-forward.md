# Proposal 039: Blocked Run Fork and Canonical Carry-Forward

| Field | Value |
|---|---|
| Date | 2026-04-12 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Current execution truth, recovery truth, artifact hierarchy, session-lineage truth, Proposal 038 |
| Goal | Introduce a server-owned continuation command for blocked runs that creates a new run from selected canonical carry-forward inputs instead of resuming the degraded blocked run directly. |

## 1. Why this proposal exists

Blocked runs often contain real work plus a degraded operational tail.

In practice that tail can include:

- repeated stalled or timed-out agent turns,
- loop counters that no longer represent useful progress,
- stale runtime session lineage,
- noisy repeated receipts and partial outputs,
- contradictory UI/report truth about whether the run is still active,
- and too much baggage for a clean continuation.

The current options are both poor:

- resume the blocked run and keep accumulating noise,
- or start a completely manual new run and lose canonical continuity.

This proposal creates a third option:

- preserve the blocked run as historical truth,
- fork a new run server-side,
- and carry forward only the selected canonical inputs that should seed the next attempt.

## 2. Outcome

After Proposal 039:

- a blocked run can be continued through a server-owned fork command,
- the old run remains immutable historical truth,
- the new run starts with a clean operational lineage,
- only selected carry-forward inputs cross the boundary,
- the relationship between the blocked run and the continuation run is explicit and queryable,
- operators no longer need to choose between stale continuation and manual reassembly.

## 3. Scope

This proposal includes:

- a blocked-run-only continuation command
- deterministic carry-forward planning
- explicit old/new run linkage
- carry-forward provenance artifacts
- UI, GraphQL, and MCP surfaces for the continuation command
- protection rules for what may and may not cross into the new run

This proposal does **not** include:

- continuing `running` runs
- mutating the blocked run's execution history
- reusing stale runtime session lineage
- carrying forward loop counters or retry debt
- replacing Proposal 038 compaction
- automatic continuation of every blocked run

## 4. Core product questions

The system must be able to answer:

1. Can a blocked run be continued without resuming its degraded execution tail?
2. Is the new run seeded only from approved canonical carry-forward inputs?
3. Can operators see exactly what was carried forward and what was intentionally dropped?
4. Does the old blocked run remain intact for audit and forensic review?
5. Can the UI and reports show that the new run is a continuation of the old one?

## 5. Eligibility

Proposal 039 applies only to runs in:

- `blocked`

It is not allowed for:

- `running`
- `ready`
- `waitingApproval`
- `pending`
- `completed`
- `failed`
- `cancelled`

### Why blocked-only

The command is meant to repair operational continuity after a run has already stopped making healthy progress.

If the run is still live, continuation is the wrong model.
If the run is terminal for a non-blocked reason, other workflows should handle it.

Blocked is the one state where:

- preserving history matters,
- resuming stale execution is risky,
- and a clean next attempt has real product value.

## 6. Core command

The system introduces one server-owned command:

## `Continue Blocked Run`

This command performs all of the following:

1. validates that the source run is currently `blocked`,
2. computes a deterministic carry-forward plan,
3. freezes the source run as immutable historical truth,
4. creates a new run with a new run ID and fresh execution lineage,
5. attaches selected carry-forward inputs to the new run,
6. records bidirectional relationship metadata between the source and continuation runs,
7. emits a continuation plan and continuation report artifact,
8. leaves the source run eligible for later Proposal 038 compaction.

There is no in-place continuation in this proposal.

## 7. Carry-forward contract

### 7.1 Carry forward only canonical inputs

The new run may carry forward:

- the latest meaningful workflow snapshot
- the latest meaningful agent catalog snapshot
- the latest valid proposal artifact
- the latest valid review corpus for the meaningful blocked frontier
- promoted or pinned artifacts that still represent live operator intent
- unresolved backlog / recovery context that still matters
- selected implementation handoff artifacts
- selected project/worktree reference metadata
- explicit operator notes added during continuation setup

### 7.2 Must not carry forward degraded operational state

The new run must not inherit:

- runtime session IDs
- active session lineage generations
- loop counters
- retry counters or retry debt
- stale transient runtime receipts
- incomplete partial outputs from failed attempts unless explicitly promoted as handoff evidence
- contradictory presentation status from the blocked run
- terminal failure metadata as if it belonged to the new run

### 7.3 Carry-forward is selective, not wholesale

The blocked run's artifact graph is not cloned blindly.

The continuation plan must explicitly decide:

- what becomes an input to the new run,
- what remains historical-only on the old run,
- what is excluded as degraded noise.

## 8. Provenance and ownership

### 8.1 The old run remains immutable

After continuation:

- the blocked run remains the source historical record,
- its artifacts and reports remain audit-visible,
- its status does not revert to `running`,
- and its execution lineage is never rewritten to pretend it was healthy.

### 8.2 The new run owns all new behavior

The new run becomes the only owner of:

- new stage transitions
- new approvals
- new runtime sessions
- new retries
- new reports
- new recovery suggestions

Carry-forward artifacts are inputs, not inherited execution history.

### 8.3 Bidirectional lineage

The system records:

- source run `continued_as -> newRunID`
- continuation run `continued_from -> oldRunID`

This linkage must be queryable in:

- run detail surfaces
- reports
- GraphQL
- MCP

## 9. Operator flow

The operator initiates continuation from a blocked run.

The system shows a deterministic carry-forward preview with:

- source run ID
- blocked frontier / last meaningful stage
- candidate carry-forward artifacts
- excluded degraded categories
- optional worktree/project reference
- warnings about anything unresolved or ambiguous

The operator then confirms continuation.

After confirmation:

- the new run is created,
- the carry-forward plan is persisted,
- the old run displays a visible `continued as` link,
- the new run displays a visible `continued from` link.

## 10. UI / GraphQL / MCP

### 10.1 UI

The UI should expose:

- `Continue Blocked Run` action on blocked run surfaces
- carry-forward preview/review sheet
- old/new continuation linkage banners
- clear distinction between historical source evidence and new-run execution truth

### 10.2 GraphQL

Add a mutation:

- `continueBlockedRun(runId: ID!, input: ContinueBlockedRunInput!): ContinueBlockedRunResult!`

Suggested result shape:

- `sourceRunId`
- `continuationRunId`
- `status`
- `planArtifactId`
- `reportArtifactId`
- `carriedArtifactIds`
- `excludedArtifactSummary`
- `warnings`

### 10.3 MCP

Add a northbound MCP tool:

- `runs.continue_blocked`

The tool should return:

- source run ID
- continuation run ID
- carry-forward summary
- warnings
- report references

## 11. Relationship to Proposal 038

Proposal 038 and Proposal 039 solve different problems.

Proposal 038:

- compacts noisy blocked or terminal runs for better inspection,
- but does not create new execution truth.

Proposal 039:

- creates fresh execution truth from a blocked run,
- but does not compact the old run by itself.

The intended operator sequence may be:

1. continue blocked run under Proposal 039,
2. later compact the old blocked run under Proposal 038.

Proposal 038 is related, but not a prerequisite for continuation.

## 12. Risks

### 12.1 Too much carry-forward

Risk:
the continuation run inherits degraded baggage and recreates the same failure pattern.

Mitigation:

- explicit exclusion rules
- plan preview before execution
- fresh runtime/session lineage only

### 12.2 Too little carry-forward

Risk:
the new run loses useful operator context and repeats old work.

Mitigation:

- deterministic candidate set
- explicit operator-visible carry-forward plan
- promoted artifacts and meaningful handoff evidence remain eligible

### 12.3 Run lineage confusion

Risk:
operators cannot tell which run is historical and which run is active.

Mitigation:

- bidirectional linkage
- visible continuation badges/banners
- reports and UI show source vs continuation roles explicitly

### 12.4 Hidden state mutation

Risk:
the system silently rewrites blocked-run truth instead of creating a fresh continuation.

Mitigation:

- blocked run remains immutable
- new run gets a new ID and new execution lineage
- carry-forward is recorded as input provenance only

## 13. Acceptance criteria

Proposal 039 is complete when:

1. a `blocked` run can be continued by one server-owned command;
2. the command creates a new run ID rather than resuming the blocked run in place;
3. the new run carries forward only the approved canonical inputs;
4. stale runtime session lineage, loop counters, and retry debt do not cross into the new run;
5. the old and new runs are explicitly linked in both directions;
6. UI, GraphQL, and MCP can surface continuation provenance and carry-forward details;
7. the old blocked run remains intact and eligible for later Proposal 038 compaction.

## 14. Final recommendation

Proposal 039 should be treated as a continuation-quality feature, not a cosmetic convenience.

When a run is blocked, the product should not force the operator to choose between:

- resuming degraded truth,
- or rebuilding context manually.

The system should provide a clean, explicit, auditable way to start the next attempt from the right inputs and nothing else.
