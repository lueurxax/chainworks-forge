# Proposal 029: MCP Northbound Control-Plane Server

| Field | Value |
|---|---|
| Date | 2026-04-01 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027 (server-side parity replica); Proposal 043 is the complementary read-path contract, not a blocker for northbound command work |
| Goal | Add an MCP server on top of the Rust + SQLite control plane so compatible agents and clients can operate the system through a stable, high-level command surface. |

## 1. Why this proposal exists

Once the server-side control plane exists, the next step is to make that control plane accessible to external agents and tools without binding the system to one specific UI.

MCP is the right northbound interface for this because it allows:
- tools,
- resources,
- prompts,
- and later MCP Apps / richer UI surfaces.

This proposal is **not** about making MCP the internal bus of the system.
MCP is the **public control facade** over the orchestrator.

## 2. Outcome

After Proposal 029:

- the system can be controlled by any MCP-compatible client,
- core orchestration remains in the Rust + SQLite control plane,
- the UI is no longer the only way to drive runs,
- the MCP surface becomes the canonical mutation interface for operators, automation clients, and agent-clients.

Proposal 029 is intentionally allowed to land early.
It does not need to wait for thin-client cutover, because command/control can stabilize before the UI read path moves.

## 3. Architectural decision

### 3.1 MCP is northbound, not internal
MCP should sit **above** the control plane.
It should expose domain actions, not internal implementation primitives.

### 3.2 Domain-first MCP surface
Tools should look like:
- `ideas.create`
- `ideas.list`
- `runs.start`
- `runs.list`
- `runs.get`
- `runs.cancel`
- `approvals.list`
- `approvals.resolve`
- `stages.retry`
- `sessions.reset_agent`
- `artifacts.get`
- `reports.compare`
- `automations.list`
- `automations.run`
- `experiments.start`

Not like:
- `set_stage_status`
- `mutate_control_plane_state`
- `attach_random_runtime_extension`

## 4. Scope

### 4.1 Tools
First-wave MCP tools:

#### Run management
- `runs.start`
- `runs.list`
- `runs.get`
- `runs.cancel`
- `runs.clone_from_snapshot`

#### Ideas
- `ideas.create`
- `ideas.list`
- `ideas.get`

#### Approvals
- `approvals.list`
- `approvals.resolve`

#### Recovery / retries
- `stages.retry`
- `agents.retry`
- `sessions.reset_agent`

#### Evidence
- `artifacts.get`
- `reports.get`
- `reports.compare`

#### Automation / operations
- `automations.list`
- `automations.run`
- `runtime.health`

#### Experiments (optional first slice)
- `experiments.list`
- `experiments.start`

### 4.2 Resources
First-wave resources:

- `run://{id}`
- `artifact://{id}`
- `report://{id}`
- `workflow://{id}`
- `approval://{id}`

### 4.3 Prompts
Prompts are optional in this proposal.
If added, they should be operator-oriented helpers, not hidden logic.

## 5. API boundaries

### 5.1 MCP owns commands
All control operations should become reachable through MCP.

“All control” means:
- starting work,
- approving work,
- retrying work,
- resetting work,
- cancelling work,
- comparing runs.

### 5.2 Read path may stay optimized
The system may keep a non-MCP read/query API for:
- high-frequency UI refresh,
- projection streams,
- paging,
- local read optimization.

MCP remains the command/control surface.

The target-state read plane is GraphQL-first.
Proposal 029 should therefore stay command-oriented and avoid becoming a second read-model protocol for the UI.

## 6. Security and authorization

Proposal 029 must define client classes.

Example:
- operator client
- agent client
- read-only observer
- experiment controller

Not every MCP client should receive the same tool set.

Minimum requirements:
- auth on MCP server
- capability-based tool exposure
- audit trail per command
- mapping from MCP caller to control-plane principal

## 7. Relationship to per-agent MCP policy

Proposal 029 does not replace per-agent runtime MCP policy.

Distinction:

- Proposal 025-style MCP policy controls what southbound agent sessions may use.
- Proposal 029 MCP server is the northbound control interface used to operate the orchestrator.

These are different layers and must remain separate.

## 8. Migration strategy

### Phase 1
- expose a minimal MCP server with runs + approvals + artifacts
- keep UI and direct service clients working

### Phase 2
- move more operator commands behind MCP
- add retry/reset/report surfaces

### Phase 3
- treat MCP as the canonical mutation/control surface

The read path may still remain outside MCP during these phases.
That separation is intentional and should later be formalized by Proposal 043.

## 9. Non-goals

Proposal 029 does **not**:
- rewrite the UI,
- move business logic into MCP,
- replace southbound runtime protocols,
- or force every high-frequency read through MCP.

## 10. Risks

### 10.1 Tool surface too broad
Risk:
- agents can bypass domain invariants.

Mitigation:
- high-level domain tools only,
- no low-level mutators.

### 10.2 MCP as internal bus
Risk:
- transport concerns leak into domain code.

Mitigation:
- keep orchestrator internal API separate,
- MCP remains a facade layer.

### 10.3 Authorization drift
Risk:
- all clients get all tools.

Mitigation:
- explicit client profiles,
- server-side capability policy.

## 11. Acceptance criteria

Proposal 029 is complete when:

1. an MCP-compatible client can start and inspect runs,
2. approvals can be listed and resolved over MCP,
3. retry/reset commands exist over MCP,
4. artifact/report retrieval works over MCP resources or tools,
5. the MCP server enforces caller-specific tool exposure,
6. control flows operate through MCP without moving orchestration logic out of the Rust control plane.

## 12. Final recommendation

Proposal 029 should make the orchestrator usable as a true control plane.

The goal is not “add MCP because MCP is trendy.”
The goal is to make orchestration accessible to any capable client while keeping product truth centralized in one place.
