# Proposal 038 Correction: Run Compaction Is MCP-Only, GraphQL Is Read-Only for Compaction State

| Field | Value |
|---|---|
| Date | 2026-04-25 |
| Status | Correction / Amendment |
| Corrects | `038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md` |
| Depends on | Proposal 072 |
| Goal | Correct P038 so `Compact Run` is an MCP-only operational command. SwiftUI may inspect compaction status and reports through GraphQL, but may not initiate compaction. |

---

## 1. Why this correction exists

P038 currently includes a GraphQL mutation for UI compaction.

That conflicts with the target boundary:

- SwiftUI uses GraphQL only,
- but SwiftUI mutations are approval-only,
- all operational commands are MCP-only.

`Compact Run` is a maintenance operation.
It archives artifacts, repairs links, rebuilds projections, and emits canonical compaction artifacts.

That must remain an MCP-controlled operator action, not a UI mutation.

---

## 2. Corrected scope

Remove from P038 in-scope list:

- `GraphQL mutation for UI`

Replace with:

- MCP tool for external operators,
- GraphQL queries/subscriptions for compaction status and reports,
- UI read surface for compaction results.

---

## 3. Corrected exposure model

## 3.1 MCP

Add/keep MCP tool:

- `runs.compact`

The MCP tool:
- validates run eligibility,
- executes compaction,
- returns compaction report identifiers,
- returns archive/dedup counts,
- returns warnings and unresolved manual-review items.

## 3.2 GraphQL

GraphQL may expose:

Queries:
- `run.compactionStatus`
- `run.compactionReports`
- `run.compactionSnapshot`
- `run.archivedArtifactSummary`

Subscriptions:
- `compactionStatusChanged(runId:)`
- or folded into existing run/artifact update subscriptions.

GraphQL must **not** expose:
- `compactRun`
- `runCompact`
- `startCompaction`
- any equivalent UI-facing compaction mutation.

---

## 4. Corrected UI behavior

SwiftUI may show:

- “This run has been compacted.”
- compaction report summary,
- archived artifact counts,
- deduplication counts,
- unresolved repair warnings,
- link to compaction snapshot,
- suggestion: “To compact this run, use MCP tool `runs.compact`.”

SwiftUI may not provide a `Compact Run` button.

---

## 5. Eligibility unchanged

Compaction remains allowed only for:

- `completed`
- `failed`
- `blocked`

Compaction remains forbidden for:

- `running`
- `ready`
- `waitingApproval`
- `pending`

---

## 6. Acceptance criteria correction

P038 is complete when:

1. `runs.compact` exists as an MCP tool;
2. no SwiftUI GraphQL mutation can trigger compaction;
3. GraphQL can display compaction status/report/snapshot;
4. compacted runs are easier to inspect;
5. compaction remains unavailable for running runs;
6. static checks prevent SwiftUI from invoking compaction.

---

## 7. Final recommendation

Compaction should remain radical and server-owned.

That is exactly why it should be MCP-only.

The UI may inspect compaction results, but it must not become a maintenance command console.
