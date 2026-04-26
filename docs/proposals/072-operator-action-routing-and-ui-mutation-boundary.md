# Proposal 072: Operator Action Routing and UI Mutation Boundary

| Field | Value |
|---|---|
| Date | 2026-04-25 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 031, Proposal 038, Proposal 044, Proposal 045, Proposal 046, Proposal 068, Target State: Rust Control Plane + ACP + GraphQL + MCP |
| Goal | Define one canonical routing matrix for operator actions so SwiftUI remains GraphQL-only with approval decisions as its only mutation path, while MCP remains the external control plane for all operational actions. |

---

## 1. Why this proposal exists

The system is moving toward a clean target state:

- Rust server owns all domain and orchestration logic.
- ACP is the southbound runtime interface to agents.
- GraphQL is the only SwiftUI API.
- MCP is the external control plane.
- SwiftUI is an observer and approval console, not an operator command center.

The current proposal set still contains drift:

- Proposal 031 currently describes a fully read-only UI with no GraphQL mutations.
- Proposal 038 currently allows a GraphQL compaction mutation.
- Proposal 046 currently includes a GraphQL reset mutation.
- Proposal 068 mostly has the right boundary but still uses language that permits UI-owned GraphQL mutations without explicitly limiting them to approvals.

That creates uncertainty about who is allowed to mutate product state.

Proposal 072 fixes this by introducing one routing rule:

> **SwiftUI GraphQL mutations are approval-only.  
> MCP owns all other operational commands.**

---

## 2. Core decision

## 2.1 SwiftUI action boundary

SwiftUI can:

- query GraphQL read projections,
- subscribe to GraphQL live updates,
- approve approval gates,
- reject approval gates.

SwiftUI cannot:

- create ideas,
- start runs,
- cancel runs,
- retry stages or agents,
- reset sessions,
- reset agent sessions,
- compact runs,
- clone runs,
- change runtime profiles,
- change context strategies,
- run experiments,
- perform recovery commands.

Those are MCP-only.

## 2.2 MCP action boundary

MCP owns external operator commands and automation commands:

- create idea,
- start run,
- cancel run,
- retry stage,
- retry agent,
- reset session,
- reset agent session,
- compact run,
- clone run,
- inspect/compare reports,
- run recovery,
- manage experiments,
- run diagnostics.

## 2.3 GraphQL read boundary

GraphQL owns UI reads and live subscriptions:

- runs,
- stages,
- approvals,
- artifacts,
- reports,
- runtime status,
- session status,
- compaction status,
- proposal metrics.

GraphQL is not a general-purpose operator command bus.

---

## 3. Canonical routing matrix

| Operation | SwiftUI via GraphQL | MCP | Notes |
|---|---:|---:|---|
| View ideas | Yes | Yes | UI reads via GraphQL; MCP can inspect externally. |
| Create idea | No | Yes | MCP-only. |
| View runs | Yes | Yes | UI reads via GraphQL; MCP can inspect externally. |
| Start run | No | Yes | MCP-only. |
| Cancel run | No | Yes | MCP-only. |
| View approval inbox | Yes | Yes | UI reads via GraphQL. |
| Approve approval | Yes | Yes | Only allowed SwiftUI mutation. |
| Reject approval | Yes | Yes | Only allowed SwiftUI mutation. |
| Retry stage | No | Yes | MCP-only. |
| Retry agent | No | Yes | MCP-only. |
| Reset session | No | Yes | MCP-only. |
| Reset agent session | No | Yes | MCP-only. |
| Resume / recover run | No | Yes | MCP-only. |
| Compact run | No | Yes | MCP-only. UI may read compaction status/report. |
| Clone / fork run | No | Yes | MCP-only. |
| Change runtime profile | No | Yes | MCP-only. |
| Change context strategy | No | Yes | MCP-only. |
| Run experiments | No | Yes | MCP-only. |
| View artifacts | Yes | Yes | UI reads via GraphQL; MCP can inspect externally. |
| View reports | Yes | Yes | UI reads via GraphQL; MCP can inspect externally. |
| View runtime health | Yes | Yes | UI reads projection; MCP can diagnose/control. |

---

## 4. Allowed GraphQL mutations

Only two UI-facing GraphQL mutations are part of the target state.

```graphql
mutation ApproveApproval($approvalId: ID!, $comment: String) {
  approveApproval(approvalId: $approvalId, comment: $comment) {
    approval {
      id
      decision
      decidedAt
    }
    run {
      id
      status
    }
  }
}
```

```graphql
mutation RejectApproval($approvalId: ID!, $reason: String!) {
  rejectApproval(approvalId: $approvalId, reason: $reason) {
    approval {
      id
      decision
      decidedAt
      comment
    }
    run {
      id
      status
    }
  }
}
```

These are not general command APIs.
They are UI-safe human gate decisions.

---

## 5. Forbidden GraphQL mutations for SwiftUI

The following must not be added to SwiftUI GraphQL usage:

- `createIdea`
- `startRun`
- `cancelRun`
- `retryStage`
- `retryAgent`
- `resetSession`
- `resetAgentSession`
- `compactRun`
- `cloneRun`
- `changeRuntimeProfile`
- `changeContextStrategy`
- `startExperiment`
- `recoverRun`

If the UI needs to show those possibilities, it may show suggested MCP actions, not execute them.

---

## 6. UI guidance for MCP-only actions

SwiftUI may display operator hints such as:

- “Run is blocked. Suggested MCP action: `runs.recover`.”
- “Session lineage appears stale. Suggested MCP action: `sessions.reset_agent`.”
- “Run has high artifact noise. Suggested MCP action: `runs.compact`.”
- “This stage can be retried through MCP: `stages.retry`.”

The UI must not execute those actions itself.

---

## 7. Required changes to existing proposals

Proposal 072 requires the following corrections:

1. Proposal 031 must allow approval-only GraphQL mutations and remove the claim that approvals are diagnostic-read-only.
2. Proposal 038 must remove GraphQL compaction mutation and make compaction MCP-only.
3. Proposal 046 must remove `resetSession` GraphQL mutation and make session reset MCP-only.
4. Proposal 068 must explicitly define GraphQL as UI read/subscription plus approval-only mutation path, while agents remain MCP-only.

---

## 8. Acceptance criteria

Proposal 072 is complete when:

1. SwiftUI uses GraphQL only.
2. SwiftUI GraphQL mutations are limited to approvals.
3. All other operator actions are MCP-only.
4. Existing proposals are corrected to match this boundary.
5. Static checks or tests can fail if SwiftUI adds forbidden GraphQL mutations.
6. Agent/operator docs no longer suggest GraphQL for non-approval operations.

---

## 9. Final recommendation

This proposal intentionally keeps the UI weak.

That is the point.

SwiftUI should remain useful for inspection and approval, but it should not become a second operator control plane.

MCP is where operational control belongs.
GraphQL is where the UI reads and resolves human gates.
