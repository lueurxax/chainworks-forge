# Target State: Rust Control Plane + ACP Runtimes + GraphQL Thin Client + MCP External Control Plane

## 1. Purpose

This document defines the desired target architecture for Chainworks Forge after the current architectural consolidation.

The system should become a **local-first control plane** with:

- a **single-process Rust server** as the only owner of domain and orchestration logic,
- **SQLite** as the local persistence layer,
- local artifact storage on disk,
- **ACP** as the southbound interface to agent runtimes,
- **GraphQL** as the only API used by the SwiftUI client,
- **MCP** as the external northbound control plane for automation, external agents, and operator commands,
- **SwiftUI** as a thin observer and approval console.

---

## 2. One-line architecture

> **Rust server = brain**  
> **SQLite + local files = local truth storage**  
> **ACP = agent runtime interface**  
> **GraphQL = only SwiftUI interface**  
> **MCP = external control plane**  
> **SwiftUI = observer + approval console**

---

## 3. Core principles

### 3.1 The Rust server owns all product truth

Only the Rust server owns:

- idea state,
- run state,
- stage state,
- approval state,
- artifact metadata,
- runtime binding truth,
- session lineage truth,
- recovery truth,
- compaction truth,
- reports and projections.

Neither the SwiftUI client, nor ACP runtimes, nor MCP clients own domain truth.

### 3.2 ACP is southbound only

ACP is the server’s interface to agent runtimes.

ACP runtimes may provide:

- session lifecycle,
- prompt execution,
- tool execution,
- streaming updates,
- runtime permissions,
- runtime receipts,
- runtime capability evidence.

ACP runtimes do **not** decide:

- which workflow stage comes next,
- which retry is legal,
- how approvals work,
- how run recovery works,
- what the canonical report truth is.

### 3.3 GraphQL is the only SwiftUI API

The SwiftUI app talks only to GraphQL.

It does not call:

- MCP,
- ACP,
- SQLite directly,
- local artifact paths directly as source of truth,
- any legacy client-owned orchestration APIs.

GraphQL is used for:

- queries,
- subscriptions,
- a minimal approval mutation surface.

### 3.4 MCP is the external control plane

MCP is the control interface for:

- external agents,
- automation,
- operator scripts,
- CLI-like control flows,
- creating ideas,
- starting runs,
- cancelling runs,
- retrying stages/agents,
- resetting sessions,
- compacting runs,
- managing experiments.

MCP is not used by the SwiftUI client.

### 3.5 The SwiftUI client is intentionally weak

The SwiftUI app should be:

- a read surface,
- a live subscription surface,
- an approval surface,
- an artifact/report viewer.

It should not be an orchestration runtime.

---

## 4. Target topology

```text
┌──────────────────────────────────────────────────────────┐
│                      SwiftUI Client                      │
│                                                          │
│  - GraphQL queries                                       │
│  - GraphQL subscriptions                                 │
│  - GraphQL approval mutations only                       │
│  - no MCP                                                │
│  - no ACP                                                │
│  - no workflow logic                                     │
└─────────────────────────────┬────────────────────────────┘
                              │
                              │ GraphQL
                              │
┌─────────────────────────────▼────────────────────────────┐
│                    Rust Local Server                     │
│                                                          │
│  Domain Engine                                           │
│  Workflow / Orchestration Engine                         │
│  Projection Engine                                       │
│  Approval Engine                                         │
│  Artifact Governance / Run Compaction                    │
│  Recovery Engine                                         │
│  Session Lineage Manager                                 │
│  ACP Runtime Manager                                     │
│  GraphQL Server                                          │
│  MCP Server                                              │
└───────────────┬─────────────────────────────┬────────────┘
                │                             │
                │                             │
        SQLite + local FS                     │ ACP
                │                             │
                │                     Claude Agent ACP
                │                     Gemini CLI ACP
                │                     Auggie ACP
                │                     Junie ACP
                │                     future ACP runtimes
                │
┌───────────────▼─────────────────────────────┐
│             Local Persistence               │
│                                             │
│  SQLite: state, projections, metadata       │
│  File store: artifacts, reports, receipts   │
└─────────────────────────────────────────────┘


External control clients / agents
        │
        │ MCP
        ▼
┌─────────────────────────────────────────────┐
│            Rust MCP Control Plane           │
└─────────────────────────────────────────────┘
```

---

## 5. Server responsibilities

## 5.1 Domain Engine

Owns the rules of the product:

- idea lifecycle,
- run lifecycle,
- workflow progression,
- stage settlement,
- approval semantics,
- retries,
- cancellation,
- recovery policy,
- artifact governance,
- compaction rules,
- report truth,
- runtime selection,
- MCP/ACP capability interpretation.

## 5.2 Workflow / Orchestration Engine

Runs inside the Rust server.

It is application-owned, not an external workflow engine.

Responsibilities:

- execute workflow state machine,
- schedule agent invocations,
- wait for approvals,
- apply retry/cancel/recovery rules,
- preserve run/stage truth,
- handle failed/blocked states,
- issue projection updates.

Non-goals:

- distributed execution,
- horizontal scaling,
- exactly-once financial-grade guarantees,
- external workflow platform integration.

## 5.3 ACP Runtime Manager

Owns interaction with agent runtimes.

Responsibilities:

- select runtime profile,
- start ACP sessions,
- attach workspace/cwd,
- attach effective MCP tool set for the agent runtime where applicable,
- collect runtime events,
- collect tool-call evidence,
- record runtime receipts,
- expose effective runtime capabilities.

The ACP Runtime Manager never owns product decisions.

## 5.4 Projection Engine

Builds server-owned read models for GraphQL.

Minimum projections:

- ideas,
- runs,
- stages,
- approvals,
- artifacts,
- reports,
- runtime status,
- active sessions,
- compaction status,
- proposal-loop metrics,
- unresolved score-lift backlog,
- recovery recommendations.

## 5.5 Artifact Governance / Run Compaction

The server owns run compaction and artifact governance.

Responsibilities:

- classify artifacts,
- archive superseded artifacts,
- exact deduplication,
- repair stale links where deterministic,
- rebuild projections,
- emit compaction reports,
- emit canonical compaction snapshots.

Compaction is allowed only for:

- `completed`,
- `failed`,
- `blocked`.

Compaction is not allowed for:

- `running`,
- `pending`,
- `ready`,
- `waitingApproval`.

Run compaction is an MCP-controlled operation, not a SwiftUI mutation.

## 5.6 GraphQL Server

GraphQL is the SwiftUI-facing API.

Responsibilities:

- queries,
- subscriptions,
- approval mutations only.

GraphQL should not become a second orchestration API.

## 5.7 MCP Server

MCP is the external control plane.

Responsibilities:

- expose domain-level tools,
- expose run/artifact/report resources,
- allow external agents to operate the system,
- provide automation entry points,
- enforce caller capability boundaries.

MCP is not an internal bus.

---

## 6. GraphQL contract

## 6.1 Queries

GraphQL queries expose read projections.

Examples:

```graphql
query {
  runs(filter: { status: RUNNING }) {
    id
    title
    status
    currentStage {
      id
      label
      status
    }
  }
}
```

Minimum query families:

- `ideas`
- `runs`
- `stages`
- `approvals`
- `artifacts`
- `reports`
- `runtimeStatus`
- `activeSessions`
- `proposalMetrics`
- `compactionStatus`

## 6.2 Subscriptions

GraphQL subscriptions are required.

Minimum subscriptions:

- active run updates,
- stage updates,
- approval inbox updates,
- runtime/session status updates,
- artifact/report availability updates,
- compaction status updates.

Subscriptions are not optional in the target state.

## 6.3 Mutations

SwiftUI may use only approval decision mutations.

Allowed GraphQL mutations:

- `approveApproval`
- `rejectApproval`

No other UI mutation is part of the target state.

### 6.3.1 Allowed mutations

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

### 6.3.2 Explicitly forbidden UI mutations

The SwiftUI client must not expose GraphQL mutations for:

- create idea,
- start run,
- cancel run,
- retry stage,
- retry agent,
- reset session,
- reset agent session,
- compact run,
- clone run,
- change runtime profile,
- change context strategy,
- change MCP/runtime configuration.

Those operations are MCP-only.

---

## 7. MCP control plane

MCP exposes external domain commands.

## 7.1 Ideas

- `ideas.create`
- `ideas.list`
- `ideas.get`

## 7.2 Runs

- `runs.start`
- `runs.get`
- `runs.list`
- `runs.cancel`
- `runs.clone`
- `runs.compact`

## 7.3 Approvals

- `approvals.list`
- `approvals.resolve`

Approvals may be resolved through MCP by authorized external operator clients, but SwiftUI also has approval mutations through GraphQL.

## 7.4 Retry / recovery

- `stages.retry`
- `agents.retry`
- `sessions.reset_agent`
- `runs.recover`
- `runs.explain_blocked`

## 7.5 Artifacts and reports

- `artifacts.get`
- `reports.get`
- `reports.compare`

## 7.6 Runtime

- `runtime.health`
- `runtime.list_profiles`
- `runtime.effective_capabilities`

## 7.7 Experiments

- `experiments.start`
- `experiments.list`
- `experiments.report`

---

## 8. UI operator boundary

## 8.1 SwiftUI can do

- view ideas,
- view runs,
- view active run state,
- view stage flow,
- view approval inbox,
- approve/reject approvals,
- view artifacts,
- view reports,
- view runtime health,
- view compaction reports,
- view suggested MCP actions.

## 8.2 SwiftUI cannot do

- create ideas,
- start runs,
- reset sessions,
- retry stages/agents,
- compact runs,
- cancel runs,
- mutate runtime profiles,
- change context strategy,
- perform recovery actions.

## 8.3 Suggested MCP actions in UI

The UI may show suggested external actions, for example:

- “Run is blocked. Suggested MCP action: `runs.recover`.”
- “Session lineage appears stale. Suggested MCP action: `sessions.reset_agent`.”
- “Run has high artifact noise. Suggested MCP action: `runs.compact`.”
- “Stage can be retried via MCP: `stages.retry`.”

The UI must not execute those actions itself.

---

## 9. Persistence

## 9.1 SQLite

SQLite stores:

- ideas,
- runs,
- stages,
- approvals,
- agent executions,
- runtime bindings,
- runtime capabilities,
- session lineage metadata,
- artifact metadata,
- report metadata,
- projections,
- compaction metadata,
- experiment metadata,
- logs metadata where useful.

## 9.2 File artifact store

The file store contains:

- proposal artifacts,
- review artifacts,
- reports,
- transcripts,
- ACP runtime receipts,
- tool-call evidence,
- compaction bundles,
- compaction reports,
- visual evidence.

SQLite stores:

- paths,
- checksums,
- artifact class,
- provenance,
- ownership,
- archive pointers,
- supersession relationships.

---

## 10. Runtime capability publication

The system does not need to hard-code a public taxonomy like:

- lifecycle-capable,
- control-capable,
- operator-grade.

However, the server must publish **effective runtime capabilities** for every runtime profile.

Examples of capability fields:

- supports session load,
- supports streaming updates,
- supports permission callbacks,
- supports tool-call visibility,
- supports MCP attach,
- supports runtime model mutation,
- supports usage telemetry,
- supports replay/history.

This allows the product to make runtime-specific decisions without pretending that all ACP runtimes are equally capable.

---

## 11. Local-first operational model

The target system is local-first.

## 11.1 Single-process singleton

The server is a single-process singleton.

There should be no local service zoo.

No required:

- Temporal,
- Redis,
- Kafka,
- NATS,
- Postgres,
- cloud deployment,
- external workflow platform.

## 11.2 Local server responsibilities

The singleton server hosts:

- domain engine,
- orchestration engine,
- GraphQL server,
- MCP server,
- ACP runtime manager,
- projection engine,
- persistence access,
- artifact governance.

## 11.3 Acceptable limitations

The target state accepts:

- local-only operation,
- no horizontal scaling,
- no multi-user guarantees,
- no financial-grade exactly-once guarantees,
- possible loss of some in-flight non-canonical state.

---

## 12. Observability

## 12.1 Product observability

Must expose:

- run status,
- stage status,
- approval status,
- blocked reasons,
- recovery suggestions,
- proposal-loop metrics,
- report status,
- compaction status.

## 12.2 Runtime observability

Must expose:

- runtime profile,
- ACP provider family,
- model/provider truth,
- effective runtime capabilities,
- session status,
- tool-call evidence,
- MCP requested/effective tool set where applicable.

## 12.3 UI observability

SwiftUI should show:

- active runs,
- approval inbox,
- run timeline,
- artifact hierarchy,
- reports,
- runtime degraded states,
- compaction status,
- suggested MCP actions.

---

## 13. Migration intent

The target state should be reached in phases.

## Phase 1 — Server parity copy

- Rust server implements a server-side copy of current logic.
- Client remains unchanged.
- Old client data remains in the client and finishes there.
- No migration of old data is required.

## Phase 2 — MCP external control plane

- MCP server exposes external control operations.
- Operator scripts can create root-backed ideas and start runs; external agents
  can create directory-free ideas and use their allowed MCP worker/read surfaces.
- Reset/retry/compact/recovery are MCP-first.

## Phase 3 — GraphQL read and live subscriptions

- GraphQL projections and subscriptions stabilize.
- UI can read server state.

## Phase 4 — Thin SwiftUI client

- SwiftUI switches to GraphQL-only.
- UI supports approvals only as mutation.
- All other operator actions remain MCP-only.

## Phase 5 — Legacy client logic removal

- Client-owned orchestration logic is removed.
- Server-owned logic becomes the only active path.

---

## 14. What is explicitly not part of the target state

The target state does not require:

- server cloud migration,
- distributed orchestration,
- Temporal,
- remote database,
- multi-node deployment,
- full audit journal as first-class projection,
- UI-driven reset/retry/compact/start/create flows,
- embedded MCP client inside SwiftUI,
- direct ACP calls from SwiftUI.

---

## 15. Success criteria

Target state is reached when:

1. the Rust server owns all orchestration/domain logic;
2. SwiftUI communicates only through GraphQL;
3. GraphQL subscriptions provide live UI state;
4. SwiftUI mutations are limited to approval decisions;
5. MCP is the external command/control plane;
6. ACP is the only strategic southbound runtime interface;
7. SQLite and local file store are sufficient for current persistence;
8. the server runs as a single-process singleton;
9. run compaction is server-owned and MCP-controlled;
10. UI can be replaced without losing workflow semantics.

---

## 16. Final statement

The target architecture is intentionally local and compact.

It avoids both extremes:

- a fat client that owns orchestration,
- and a distributed backend platform with unnecessary infrastructure.

The desired end state is:

> a local Rust control plane with SQLite, ACP agent runtimes, GraphQL thin UI, and MCP external control.

This keeps the system powerful, automatable, and inspectable while making the client dramatically simpler.
