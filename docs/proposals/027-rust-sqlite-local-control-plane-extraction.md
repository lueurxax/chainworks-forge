# Proposal 027: Rust + SQLite Local Control Plane Extraction

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Supersedes | Previous draft: `027-go-temporal-control-plane-extraction.md` |
| Depends on | Current product semantics already established in the client: execution truth, contract truth, session lineage, proposal-loop fidelity, MCP policy, ACP direction |
| Goal | Move orchestration and domain logic out of the client into a local Rust control-plane daemon backed by SQLite, while preserving current product semantics and keeping the client functional during the transition. |

## 1. Why this proposal exists

The client currently owns too much:

- run lifecycle
- stage transitions
- retries
- approvals
- report truth
- recovery logic
- session lineage behavior
- proposal-loop orchestration
- runtime integration glue

That makes the UI heavy, the architecture hard to evolve, and every runtime change more expensive than it should be.

The next architectural step is still correct:

> move product logic out of the client and into a proper control-plane service.

What changes in this rewrite is the substrate.

Instead of introducing a full external workflow platform, this proposal assumes:

- the system should remain local-first,
- deployment should stay as small as possible,
- one daemon + one database is preferable to a multi-component orchestration stack,
- high availability and strong distributed guarantees are not the current priority,
- losing some in-flight state is acceptable compared with introducing major platform overhead.

That makes **Rust + SQLite** a better fit than **Go + Temporal** for the current stage of the product.

This proposal does **not** redesign product semantics.
It moves existing semantics into a smaller, local-first control-plane architecture.

---

## 2. Outcome

After Proposal 027:

- the client is no longer the owner of orchestration logic,
- a local Rust daemon becomes the owner of workflow truth,
- SQLite becomes the durable local source of truth for runs, stages, approvals, artifacts, and read models,
- background orchestration is driven by an application-owned workflow engine rather than a third-party workflow platform,
- the client becomes a thin consumer of projections and commands,
- no existing run/report/recovery semantics should regress.

This proposal intentionally chooses a simpler topology over stronger distributed guarantees.

---

## 3. Core architectural decision

### 3.1 A local Rust daemon owns orchestration

A single local Rust daemon should own:

- workflow progression
- waiting on approvals
- retries
- restart reconciliation
- long-lived stage transitions
- cancellation
- projection updates
- runtime adapter coordination

This daemon is the local control plane.

It is not a remote server by default.
It should be able to run alongside the desktop application on the same machine.

### 3.2 SQLite owns durable local truth

SQLite should become the persistent store for:

- runs
- stages
- agent executions
- approvals
- artifacts metadata
- session lineage metadata
- projection/read models
- command journal / orchestration events
- recovery markers

SQLite is chosen because:

- it preserves the single-machine, local-first deployment model,
- it avoids introducing another service just for persistence,
- it is good enough for the current scale and reliability needs,
- it simplifies packaging, startup, and troubleshooting.

### 3.3 The workflow engine is application-owned

This proposal explicitly avoids introducing an external workflow engine such as Temporal.

Instead, the Rust daemon should implement a **product-owned orchestration engine** with:

- explicit persisted run state,
- explicit stage transitions,
- idempotent command handlers,
- restart repair,
- retry scheduling,
- approval waits,
- background execution loops,
- projection updates.

This is a conscious trade:

- less platform power,
- much less infrastructure weight,
- more direct control over semantics,
- easier local deployment.

### 3.4 The client stops owning logic

The client should stop deciding:

- what stage comes next,
- how retries work,
- how approvals mutate state,
- how session resets are applied,
- what recovery paths are legal.

The client should eventually become:

- query renderer,
- artifact viewer,
- approval/retry/reset command initiator.

---

## 4. Proposed system shape

```text
Client UI
  -> query/read API
  -> command API

Local Rust Control Plane Daemon
  -> orchestration engine
  -> background executor
  -> projections/read models
  -> artifact metadata layer
  -> runtime adapter layer
  -> command journal / recovery layer

SQLite
  -> source of truth
  -> projections
  -> recovery state
  -> job scheduling state

Southbound runtimes
  -> Goose legacy adapter
  -> future ACP adapters
  -> provider-specific adapter layer
```

This is intentionally a **small local system**:

- one daemon
- one database
- no external orchestration platform
- no required distributed deployment

---

## 5. Rust daemon model

### 5.1 Process model

The Rust daemon should run as a local control-plane process.

Recommended characteristics:

- single binary
- local IPC or localhost API
- owns background loops
- owns SQLite connection pool
- can be started by the desktop client or launched independently
- restart-safe through persisted state in SQLite

### 5.2 Main subsystems

The daemon should contain at least:

#### Command handlers
Responsible for:
- start run
- approve/reject
- retry stage/agent
- reset session lineage
- cancel run
- clone from snapshot

#### Orchestration engine
Responsible for:
- evaluating current run state
- deciding legal next actions
- enqueuing background work
- waiting on approvals
- progressing stage lifecycles

#### Background executor
Responsible for:
- polling runnable work
- invoking runtime adapters
- persisting receipts and outcomes
- updating stage and run settlement state

#### Projection updater
Responsible for:
- materializing UI-friendly read models
- keeping summary tables fresh
- supporting reports and recovery views

#### Recovery and repair layer
Responsible for:
- startup reconciliation
- stale active record repair
- rehydration after process restart
- marking unverifiable state explicitly

### 5.3 Concurrency model

This system does not need distributed-scale coordination right now.

A reasonable local-first model is:

- one SQLite database,
- one daemon process,
- in-process task scheduler,
- transactions around state mutation,
- row-level ownership or lease fields where needed,
- optimistic idempotency for retries and restart repair.

No attempt should be made in Proposal 027 to optimize for multi-host scale.

---

## 6. Persistence model

### 6.1 SQLite as source of truth

SQLite should store both:

- canonical domain truth
- query-friendly projections

The system does **not** need a separate event store or separate workflow platform state store in this proposal.

### 6.2 Suggested table groups

#### Canonical execution tables
- `runs`
- `stage_executions`
- `agent_executions`
- `approvals`
- `artifacts`
- `session_lineages`
- `aggregate_settlements`

#### Orchestration support tables
- `command_journal`
- `work_items`
- `background_leases`
- `startup_repairs`
- `runtime_invocations`

#### Projection/read tables
- `run_summaries`
- `stage_summaries`
- `approval_inbox`
- `artifact_index`
- `proposal_loop_metrics`
- `recovery_recommendations`

### 6.3 Command journal

The daemon should record incoming mutating commands in a durable command journal:

- command id
- timestamp
- actor/principal
- target entity
- command type
- request payload
- result status
- linked execution ids

This is not full event-sourcing.
It is a practical audit and repair layer.

### 6.4 Work queue

The daemon should maintain a SQLite-backed `work_items` table for runnable internal jobs, for example:

- invoke agent execution
- compute aggregate summary
- rebuild score-lift backlog
- validate coverage
- rebuild report
- apply startup repair
- trigger next stage transition

This queue is application-owned and domain-specific, not generic infrastructure.

---

## 7. Workflow engine model

### 7.1 No generic workflow DSL engine in this proposal

Proposal 027 does **not** attempt to build a universal workflow runtime.

It builds a **product-owned orchestration engine** tuned to existing Chainworks semantics.

That means:

- explicit run state machines,
- explicit stage transitions,
- explicit recovery rules,
- explicit approval waits,
- explicit retry legality,
- explicit background work categories.

### 7.2 State progression approach

Recommended execution model:

1. client issues command
2. command handler validates and persists command
3. command handler mutates canonical state or enqueues work
4. background executor processes work item
5. executor persists outcome
6. orchestration engine re-evaluates run and stage state
7. projection updater refreshes read models

This keeps orchestration explicit and debuggable.

### 7.3 Restart behavior

On daemon startup:

- load active/incomplete runs,
- repair stale work items,
- repair orphaned approvals or active states,
- re-enqueue safe pending work,
- mark unverifiable situations explicitly as blocked/review-required.

The goal is not perfect durability.
The goal is **predictable local recovery**.

---

## 8. API boundary

### 8.1 Client-service boundary

The Rust daemon should expose a local command/query interface.

The exact transport may be:

- localhost HTTP
- Unix domain socket
- named pipe
- embedded RPC

Proposal 027 does not force the final choice, but it does require a clean boundary.

### 8.2 Ownership rules

The service owns:
- workflow truth
- retries
- approvals
- stage settlement
- report-building triggers
- runtime adapter choice
- recovery legality

The client owns:
- rendering
- local view state
- user interaction
- presentation-only composition

---

## 9. Migration strategy

### Phase 1 — carve out domain and persistence seams
- stop adding new orchestration logic to the client
- define service-owned domain contracts
- normalize database-backed truth surfaces
- isolate runtime adapters behind interfaces

### Phase 2 — implement local Rust daemon
- command handlers
- SQLite persistence
- work queue
- orchestration engine
- projections
- startup repair
- bridge to existing runtime adapters

### Phase 3 — route one bounded slice through the daemon
- proposal loop or another narrow live slice
- client remains functional
- validate parity on real runs

### Phase 4 — move remaining orchestration ownership out of client
- all execution commands route through daemon
- client becomes projection/command consumer

### Phase 5 — remove obsolete client-owned orchestration code
- delete duplicated orchestration logic
- keep only UI-local state and presentation helpers

---

## 10. Why not Temporal right now

Temporal remains a strong platform for durable orchestration.

It is not chosen here because current product constraints are different:

- local-first deployment matters more than distributed durability,
- one daemon + one database is preferred,
- strong guarantees are not yet worth the platform overhead,
- losing some in-flight state is acceptable,
- scale and multi-node orchestration are not current priorities.

The design choice here is:

> use a smaller local system now, keep the architecture clean, and leave room for a future move to a stronger workflow platform if the product grows into that need.

---

## 11. Non-goals

Proposal 027 does **not**:

- add MCP northbound server support,
- rewrite the UI,
- force ACP migration,
- remove Goose,
- solve runtime-provider comparison,
- optimize for multi-node scale,
- implement strong distributed guarantees,
- or package the final deployment topology for remote hosting.

Those belong to later proposals.

---

## 12. Risks

### 12.1 Reinventing a workflow engine poorly
Risk:
- building a fragile homegrown scheduler/orchestrator.

Mitigation:
- keep scope narrow,
- map directly to current product semantics,
- avoid inventing a generic workflow platform,
- use explicit tables and transitions,
- treat restart repair and idempotency as first-class.

### 12.2 SQLite misuse
Risk:
- contention, locking surprises, hidden concurrency problems.

Mitigation:
- keep deployment single-machine,
- keep daemon authoritative,
- keep concurrency modest,
- use transactions deliberately,
- avoid pretending the database is distributed.

### 12.3 Split-brain truth during migration
Risk:
- client and daemon both think they own run truth.

Mitigation:
- authority transfer plan,
- feature-flagged slices,
- no new business logic added to the client.

### 12.4 Projection drift
Risk:
- read models diverge from canonical state.

Mitigation:
- projections treated as derived state,
- rebuild tooling,
- audit links between work items, commands, and projections.

---

## 13. Acceptance criteria

Proposal 027 is complete when:

1. orchestration logic is executable in the Rust daemon,
2. SQLite is the durable local source of truth for at least one real workflow slice,
3. the client can render run/stage/approval/artifact state entirely from service projections,
4. approval and retry flows work via service commands,
5. current product semantics do not regress,
6. the system can survive process restart with predictable local repair,
7. the client is no longer the owner of workflow decisions,
8. the full local topology remains “daemon + SQLite” without requiring a separate workflow platform.

---

## 14. Final recommendation

Proposal 027 should be treated as a local-first control-plane extraction, not as a platform migration.

The goal is not to chase infrastructure sophistication.
The goal is to move workflow truth out of the client and into a small, explicit, maintainable local system.

Rust + SQLite is the right choice here if the priorities are:

- local deployment,
- minimal moving parts,
- understandable ownership,
- acceptable tradeoff on durability guarantees,
- and the ability to evolve into something stronger later if the product truly needs it.
