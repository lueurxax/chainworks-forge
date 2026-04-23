# Proposal 068: Agent MCP Primary Control Plane and GraphQL UI Boundary

| Field | Value |
|---|---|
| Date | 2026-04-23 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [029-mcp-northbound-control-plane-server.md](029-mcp-northbound-control-plane-server.md), [031-thin-ui-rewrite-over-projections-and-mcp.ru.md](031-thin-ui-rewrite-over-projections-and-mcp.ru.md), [044-idea-crud-completeness-and-lifecycle-mcp-tools.md](044-idea-crud-completeness-and-lifecycle-mcp-tools.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [063-mcp-tool-response-shaping-and-field-selection.md](063-mcp-tool-response-shaping-and-field-selection.md), [064-run-worktree-main-sync-and-cross-run-knowledge-transfer.md](064-run-worktree-main-sync-and-cross-run-knowledge-transfer.md), [065-operator-retry-instruction-contract.md](065-operator-retry-instruction-contract.md), [067-lead-decomposed-implementation-slices-and-capability-minimal-agent-routing.md](067-lead-decomposed-implementation-slices-and-capability-minimal-agent-routing.md), [mcp-northbound-control-plane-server.md](../reference/mcp-northbound-control-plane-server.md), [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md) |
| Scope | Make MCP the complete primary surface for agents, automations, and operator-debug tooling, while reserving GraphQL for the macOS UI read path. Agents must not use GraphQL or direct SQLite reads/writes for normal Chainworks operations. |
| Goal | An agent can inspect, operate, recover, retry, approve, cancel, clean up, and diagnose Chainworks runs through MCP alone, with durable audit and compact tool results, without falling back to GraphQL queries or ad-hoc database inspection. |

**Gate naming note:** this proposal owns the future canonical gate alias `proposal-068|p068`. It must be added to `scripts/test-gate.sh` and `docs/reference/test-gates.md` when implementation starts.

---

## 1. Context and Motivation

P031 correctly sets the macOS app direction: the UI reads server-owned truth through GraphQL projections and does not reconstruct workflow state locally. That is the UI boundary.

The agent boundary is different. Agents already live inside MCP-capable environments and should operate Chainworks through MCP tools/resources, not by becoming GraphQL clients and not by reading SQLite tables. Recent dogfood operation exposed the gap: routine agent work used GraphQL and direct DB queries for:

- daemon and scheduler health;
- run status and blocked-state inventory;
- cancellation;
- stale active-execution diagnosis;
- worktree cleanup decisions;
- run artifact lookup;
- retry/approval decisions;
- post-restart state assessment.

That is backwards for agent automation. GraphQL is for the UI. SQLite is the daemon's private persistence implementation. MCP is the northbound agent/operator automation contract.

The current system has pieces of the target:

- P044 covers idea lifecycle MCP tools.
- P045 covers recovery/retry MCP tools.
- P063 covers compact MCP response shaping.
- P064 covers run worktree sync and knowledge transfer.
- P065 covers operator retry instructions.
- P067 covers capability-minimal agent routing.

No proposal currently binds these into one rule: **agents must be able to do the work through MCP, and if they cannot, the missing MCP capability is a product gap rather than permission to use GraphQL or SQLite.**

---

## 2. Non-Negotiable Boundary

### 2.1 GraphQL is UI-only

GraphQL is the macOS operator UI read path and subscription path. It may also be used by UI test harnesses and developer schema tests, but it is not an agent operations API.

Agents, CLI automations, scheduled monitors, and Codex/Claude/Gemini work sessions must not use GraphQL for routine Chainworks control-plane work.

### 2.2 MCP is the agent control and inspection surface

MCP must cover:

- compact reads for current state;
- command execution;
- recovery actions;
- artifact/report inspection;
- scheduler and daemon health;
- run-owned worktree lifecycle;
- cleanup/housekeeping;
- structured diagnostics.

MCP tools/resources can internally read the same repositories that GraphQL reads. The caller contract is MCP.

### 2.3 SQLite is private implementation detail

Direct SQLite access is not a normal agent workflow. It is allowed only as a break-glass developer diagnostic while this proposal is unimplemented, and every such use is evidence of a missing MCP tool/resource.

After P068, direct DB inspection by an agent should be unnecessary for ordinary run orchestration, status reporting, retry, cleanup, or audit preparation.

### 2.4 No hidden write path through GraphQL for agents

Any agent-write operation must be represented as an MCP command tool with:

- principal/capability policy;
- typed params and typed errors;
- command journal caller context;
- returned `journal_id` where a command changes durable state;
- idempotency or duplicate-call semantics where retries are expected.

GraphQL mutations may remain for UI-owned flows only when explicitly required by a UI proposal. They are not the agent fallback.

---

## 3. Product Questions This Proposal Must Answer

1. Can an agent show "what is happening now" without GraphQL or SQLite?
2. Can an agent cancel, retry, approve, reject, resume, or re-arm a run through MCP only?
3. Can an agent diagnose blocked runs, stale executions, queue pressure, daemon health, and cleanup needs through MCP only?
4. Can an agent safely remove run-owned worktrees/artifacts for cancelled or closed-out runs without direct filesystem guessing?
5. Can an agent inspect proposal/run artifacts through MCP resources without scanning `.chainworks/runs` manually?
6. Can every MCP read response fit LLM tool-result limits by default, while still offering explicit includes for full evidence?
7. Can the daemon deny GraphQL access to agent principals without breaking the macOS UI?
8. Can docs and prompts stop telling agents to use GraphQL hints or SQLite snippets for normal operation?

---

## 4. Scope

### In scope

- A binding policy: **agent principals use MCP only; UI principals use GraphQL for UI reads.**
- Agent-facing MCP parity for the operational tasks currently forcing GraphQL/SQLite fallback.
- Principal-class policy that can distinguish UI GraphQL callers from agent MCP callers.
- MCP tool/resource additions listed in the gap matrix below.
- Response-shaping compatibility with P063 so MCP remains LLM-consumable.
- Audit/journal behavior for mutating tools.
- A documentation pass that removes agent-facing GraphQL/SQLite workarounds from runbooks, prompts, and proposal text.
- A gate that fails when agent docs or tests require GraphQL/SQLite for ordinary operation.

### Out of scope

- Replacing GraphQL as the macOS UI read path.
- Removing GraphQL schema tests or UI GraphQL queries.
- Exposing raw SQL over MCP.
- Adding unrestricted "admin shell" MCP tools.
- Making MCP mirror every GraphQL field one-for-one. MCP should expose task-shaped compact tools/resources, not a second GraphQL schema.
- Hiding SQLite from daemon internals. The restriction is about caller boundary, not implementation.

---

## 5. MCP Gap Matrix

### 5.1 Status and health

| Current fallback | Required MCP surface | Notes |
|---|---|---|
| GraphQL `daemonStatus`, `schedulerHealthSummary` | `daemon.status`, `scheduler.health`, `resource: chainworks://daemon/status`, `resource: chainworks://scheduler/health` | Compact default with daemon pid/start/schema/build, queue depth, active executions, stale flag, backpressure state, DB writer pressure. |
| SQLite `runs` status counts | `runs.summary` | Counts by run status, current workflow state, proposal key, provider family, and terminal/non-terminal class. |
| SQLite `work_items` pending/running queries | `work_items.summary`, `work_items.list` | List only compact identifiers, kind, run/stage, status, age, last error summary. Full payload only via explicit include. |
| SQLite `agent_executions` running rows | `agent_executions.list` | Must expose running/stale candidates, provider family, agent id, run id, stage id, started age, paired work item if any. |

### 5.2 Run control and recovery

| Current fallback | Required MCP surface | Owner relationship |
|---|---|---|
| GraphQL `cancelRun` | Existing `runs.cancel` must be the agent path; GraphQL cancel is UI-only/deprecated for agents | P029/P031/P068 |
| GraphQL or manual approval mutation | Existing `approvals.resolve`; agents must use MCP only | P031/P068 |
| Stage retry through mixed GraphQL/debug state | Existing `stages.retry` plus P065 `operator_instruction` support | P045/P065/P068 |
| Manual stale targeted retry analysis | `agents.retry`, `runs.resume`, `recovery.suggest`, `recovery.evidence` | P045, extended by P068 if gaps remain |
| Direct DB stale execution inspection | `recovery.stale_executions`, `recovery.repair_projection` | P068 adds explicit diagnostic/repair path. Repair tool must be guarded and journaled. |

### 5.3 Worktree and run-owned cleanup

| Current fallback | Required MCP surface | Notes |
|---|---|---|
| `git worktree list` + manual path matching | `worktrees.list`, `worktrees.get` | Return run id, branch, path, dirty count, main ancestor, disk usage, lifecycle owner. |
| Manual commit-before-merge preservation | `worktrees.preserve_changes` | Creates a durable commit or patch bundle before lifecycle cleanup. Must return preservation proof. |
| Manual `git merge main` per run worktree | `worktrees.merge_main` | Uses P064 policy, conflict reporting, and no destructive cleanup. |
| Manual `git worktree remove` | `worktrees.remove` | Allowed only for terminal/cancelled/closed-out runs with durability proof. |
| Manual artifact dir deletion | `runs.cleanup` | Deletes only run-owned artifact directories and generated state after terminal/cancelled closeout; returns deleted paths and bytes freed. |
| Manual branch pruning | `branches.delete_run_branch` | Only when branch is merged or explicitly preserved. Must refuse dirty/unmerged branches unless operator confirms with proof. |

### 5.4 Artifacts, reports, and proposal truth

| Current fallback | Required MCP surface | Notes |
|---|---|---|
| Filesystem reads under `.chainworks/runs` | `artifacts.list`, `artifacts.get`, `artifacts.read_text` | Resources must include proposal/current/approved, review, implementation, audit, summaries, and operator notes. |
| Manual report lookup | Existing `reports.get` plus `report://{run_id}` parity | Must cover compact execution truth and full evidence via include. |
| Manual proposal copy from run artifact to docs | `proposals.export_current`, `proposals.export_approved` | Writes repo docs only when asked and returns diff/target path. This is a command tool with journal id. |
| Score trajectory from overwritten files | P063 `reviews.score_trajectory` | Must remain MCP-first; GraphQL mirror is UI-only. |

### 5.5 Daemon maintenance and housekeeping

| Current fallback | Required MCP surface | Notes |
|---|---|---|
| Manual DB quick_check/checkpoint/VACUUM | `maintenance.db_health`, `maintenance.checkpoint`, `maintenance.vacuum` | Guarded operator/debug tools; agents can request status, operators can run maintenance. |
| Manual generated-state cleanup | `maintenance.generated_state_summary`, `maintenance.cleanup_generated_state` | Must respect protected paths and return dry-run by default. |
| Manual process cleanup | `runtime.processes.list`, `runtime.processes.cleanup_stale` | Must not trigger Xcode approval prompts; should identify ACP/MCP/provider processes by run/session ownership. |
| Manual daemon restart assessment | `daemon.restart_plan`, `daemon.restart` | Restart should be an explicit MCP command with active-work loss estimate, not shell-only habit. |

### 5.6 MCP tool discoverability

Every new MCP tool must include:

- task-oriented description;
- compact default response budget;
- `include` list for heavy fields where needed;
- typed error code and recovery hint;
- `journal_id` for mutating tools;
- capability id in `CapabilityToolId`;
- principal policy tests;
- parity resource where a URI shape is better for browsing.

---

## 6. Principal and Transport Policy

### 6.1 Principal classes

P068 introduces or formalizes these caller categories:

| Caller | Intended surface | Notes |
|---|---|---|
| `ui_operator` | GraphQL + UI-owned MCP commands if still needed | The macOS app token. |
| `agent_operator` | MCP only | Codex/Claude/Gemini agents acting for the operator. |
| `automation` | MCP only | Heartbeats, scheduled monitors, runbook automation. |
| `observer` | MCP compact reads only | No mutating tools. |
| `developer_breakglass` | Explicit local debug only | May use SQL/shell outside normal operation; not granted to agents by default. |

If the existing `PrincipalClass` names cannot be extended without churn, P068 may implement the split as capability sets attached to existing classes first, then introduce named classes later. The behavior is binding even if the first implementation uses existing enum names.

### 6.2 GraphQL agent denial

The GraphQL auth layer must be able to reject non-UI principals. Agent tokens must not be accepted by `/graphql` or `/graphql/ws` for normal operation.

GraphQL tests should prove:

- UI principal can execute UI queries/subscriptions.
- Agent principal gets a typed forbidden/unauthorized response from GraphQL.
- Agent principal can perform the equivalent supported task through MCP.

### 6.3 Documentation and prompt policy

Agent-facing docs, runbooks, and proposal examples must not recommend:

- `curl /graphql`;
- direct `sqlite3 .chainworks/control-plane.db`;
- manually reading tables for routine status;
- GraphQL hints as MCP fallback for agents.

Where GraphQL examples remain, they must be explicitly labeled "UI/client schema example" or "developer schema test", not "agent operation".

---

## 7. Implementation Plan

### Phase 1: Inventory and docs contract

1. Add P068 proposal and update P063 to remove agent-facing GraphQL fallback language.
2. Inventory every current MCP tool/resource and map it to the gap matrix.
3. Add a docs lint that flags agent-facing GraphQL/SQLite operational snippets outside allowlisted UI/developer sections.

### Phase 2: Read parity for operational status

1. Add `daemon.status`, `scheduler.health`, `runs.summary`, `work_items.summary`, `work_items.list`, and `agent_executions.list`.
2. Make default envelopes compact and P063-compatible.
3. Add resource templates for daemon/scheduler status where useful.

### Phase 3: Recovery and cleanup parity

1. Complete P045/P065 surfaces that are needed for current blocked-run work.
2. Add `recovery.stale_executions` and guarded `recovery.repair_projection`.
3. Add `runs.cleanup`, `worktrees.*`, and branch/run-owned artifact cleanup tools with preservation proof.

### Phase 4: Maintenance and process hygiene

1. Add `maintenance.db_health`, checkpoint/vacuum controls, and generated-state cleanup.
2. Add `runtime.processes.list` and `runtime.processes.cleanup_stale`.
3. Add `daemon.restart_plan` before any MCP restart command.

### Phase 5: Enforce boundary

1. Deny GraphQL access for agent/automation principals.
2. Update dogfood agent configs so they receive MCP credentials, not GraphQL instructions.
3. Add gate coverage proving a representative agent workflow completes without GraphQL or direct DB.

---

## 8. Acceptance Criteria

P068 is complete when:

1. An agent can produce the standard run-status report using only MCP:
   - daemon health;
   - scheduler health;
   - blocked/running/cancelled counts;
   - per-run state;
   - active work items;
   - active/stale agent executions.
2. An agent can cancel/cleanup a terminal or intentionally abandoned run using only MCP, with durability proof before worktree removal.
3. An agent can retry or resume blocked runs using MCP tools and P065 retry instructions, without editing artifacts by hand.
4. An agent can inspect run artifacts/reports/proposal outputs through MCP resources/tools.
5. Agent principal tokens are rejected by GraphQL.
6. The proposal gate fails if agent-facing docs or tests include unallowlisted GraphQL/SQLite operational snippets.
7. The macOS UI still uses GraphQL for its read path and is not forced through MCP reads.
8. Direct SQLite access is documented only as break-glass developer diagnostics, not normal agent procedure.

---

## 9. Validation

Add a canonical gate alias:

```bash
./scripts/test-gate.sh proposal-068
```

Minimum proof lanes:

- `proposal-068-mcp-status`: MCP daemon/scheduler/run/work-item/agent-execution status tools return compact envelopes.
- `proposal-068-boundary`: agent principal denied on GraphQL and accepted on MCP equivalent.
- `proposal-068-cleanup`: terminal run cleanup requires durability proof and deletes only run-owned paths.
- `proposal-068-no-db-docs`: docs lint rejects agent-facing direct SQLite/GraphQL operational examples outside allowlisted sections.
- `proposal-068-response-budget`: P063 budget tests cover every new read-shaped MCP tool.

---

## 10. Open Questions

1. Should the first implementation introduce new principal class names (`ui_operator`, `agent_operator`) or encode the split as capability sets on existing classes?
2. Should `daemon.restart` be in P068, or should P068 stop at `daemon.restart_plan` and leave actual restart to a separate daemon supervisor proposal?
3. Should `runs.cleanup` archive deleted run-owned artifacts into a compressed bundle before deletion, or is preservation proof limited to worktree/code changes?
4. Should MCP expose a generic `queries.explain_missing_tool` helper for cases where an agent knows the task but not the tool, or is `tools/list` plus docs enough?

