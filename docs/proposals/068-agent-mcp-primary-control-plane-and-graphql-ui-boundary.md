# Proposal 068 Correction: MCP Primary Control Plane and GraphQL Approval-Only UI Boundary

| Field | Value |
|---|---|
| Date | 2026-04-25 |
| Status | Correction / Amendment |
| Corrects | `068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md` |
| Depends on | Proposal 072 |
| Goal | Update P068 so it reflects the final target boundary: agents and automations use MCP only; SwiftUI uses GraphQL for reads, subscriptions, and approval-only mutations. |

---

## 1. Why this correction exists

P068 mostly has the right boundary:

- agents should not use GraphQL,
- agents should not inspect SQLite,
- MCP is the agent/operator automation surface,
- GraphQL is for the UI.

But P068 currently phrases GraphQL as primarily the UI read path and only loosely allows UI mutations when explicitly required.

The target state is now sharper:

> SwiftUI uses GraphQL for reads, subscriptions, and approval-only mutations.  
> All non-approval operator actions are MCP-only.

---

## 2. Corrected non-negotiable boundary

## 2.1 GraphQL is UI-only

GraphQL is the macOS UI path for:

- reads,
- subscriptions,
- approval-only mutations.

GraphQL is not an agent operations API.

Agents, CLI automations, scheduled monitors, and external operator scripts must not use GraphQL for routine Chainworks work.

## 2.2 MCP is the external control plane

MCP must cover:

- creating ideas,
- starting runs,
- cancelling runs,
- retrying stages/agents,
- resetting sessions,
- compacting runs,
- running recovery actions,
- inspecting artifacts/reports,
- diagnostics,
- cleanup/housekeeping,
- experiments.

## 2.3 SQLite is private

Direct SQLite access remains a break-glass developer diagnostic only.

## 2.4 GraphQL mutation boundary

GraphQL mutations may exist for UI approvals only:

- `approveApproval`
- `rejectApproval`

No other GraphQL mutation should be used by SwiftUI.

Agents must never use GraphQL mutations.

---

## 3. Corrected principal policy

| Principal | Allowed surface |
|---|---|
| `ui_operator` | GraphQL queries, subscriptions, approval-only mutations |
| `agent_operator` | MCP only |
| `automation` | MCP only |
| `observer` | MCP compact reads only or GraphQL read-only if explicitly provisioned as UI-like observer |
| `developer_break_glass` | Explicitly logged debug access only |

---

## 4. Corrected gap matrix implications

Update P068 matrix rows:

- Approval decisions:
  - UI may resolve approvals via GraphQL approval mutations.
  - Agents resolve approvals via MCP `approvals.resolve`.

- Cancel:
  - MCP-only.
  - No UI GraphQL cancel.

- Retry:
  - MCP-only.
  - No UI GraphQL retry.

- Reset:
  - MCP-only.
  - No UI GraphQL reset.

- Compact:
  - MCP-only.
  - No UI GraphQL compact.

- Create/start:
  - MCP-only.
  - No UI GraphQL create/start.

---

## 5. Corrected out-of-scope

P068 still does not:

- replace GraphQL for the macOS UI;
- force UI through MCP;
- expose raw SQL over MCP;
- mirror every GraphQL field in MCP.

P068 also does not create UI GraphQL mutations except approvals.

---

## 6. Acceptance criteria correction

P068 is complete when:

1. agent principals are denied GraphQL;
2. agents can perform routine operations through MCP only;
3. UI principals can use GraphQL reads/subscriptions;
4. UI principals can use only approval mutations;
5. all non-approval operator commands are MCP-only;
6. docs and prompts stop suggesting GraphQL/SQLite for agents.

---

## 7. Final recommendation

P068 should remain the proposal that protects the northbound boundary.

The corrected boundary is:

> Agents and automations use MCP.  
> UI uses GraphQL.  
> UI writes are approvals only.
