# Proposal 081: Boundary-First API and Auth Contract Matrix

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Draft |
| Author | Codex |
| Depends on | [068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md](068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md), [ui-action-boundary.md](../reference/ui-action-boundary.md), [current-system-baseline.md](../reference/current-system-baseline.md) |
| Related | P031, P072, PR #3 review findings from 2026-04-29 |
| Scope | Define a single executable boundary matrix for caller identity, transport, authoritative record, allowed actions, deny behavior, read-model deltas, and proof tests across MCP, GraphQL, auth, approvals, and UI affordances. |
| Goal | Stop boundary drift before another UI-write or approval-actionability slice by making every caller/transport/action combination explicit and testable. |

---

## 1. Problem

Recent review evidence shows the same boundary weakness appearing in multiple forms:

- policy exists in `control-plane/crates/auth/src/lib.rs`, but callers can still drift across token resolution, GraphQL gates, MCP gates, subscription gates, and approval actionability;
- GraphQL approval-only UI rules are documented, but legacy/control mutation exposure and compatibility fixtures make the actual allowed surface hard to audit;
- approval provenance can be caller-supplied in some flows where the durable source of truth should own it;
- Swift UI affordances depend on GraphQL actionability fields that can contradict backend authorization.

This is not a feature gap. It is a contract gap: each surface has local policy, but there is no one-page matrix that all code paths, tests, and docs must satisfy.

## 2. Decision

Before adding any new UI write, approval action, GraphQL mutation, MCP command, or automation caller, the repository must contain a checked-in boundary matrix.

The matrix is authoritative for northbound access behavior. Code may be stricter than the matrix during rollout, but code must not be broader.

## 3. Boundary Matrix

Add canonical artifact:

```text
docs/reference/boundary-first-api-auth-contract.md
```

Required rows:

| Caller | Transport | Authoritative record | Allowed actions | Deny behavior | Read-model delta | Required tests |
|---|---|---|---|---|---|---|
| `ui_operator` | GraphQL query/subscription | DB projection/read model | reads, subscriptions | typed GraphQL auth error or redacted field | freshness/actionability fields remain read-only | query, subscription, redaction |
| `ui_operator` | GraphQL mutation | approval record + auth policy | `approveApproval`, `rejectApproval` only | deny all non-approval mutations before resolver side effects | approval actionability changes after settlement | allowed approval, denied non-approval |
| `agent_operator` | MCP | command journal + command handler | operator automation commands | typed MCP capability denial | MCP response includes reason and stable ids | allowed MCP command, denied GraphQL |
| `automation` | MCP | command journal + command handler | scoped automation commands | typed MCP capability denial | command receipt/projection update | token-scope matrix |
| `observer` | GraphQL read-only or MCP compact reads | projection/read model | read-only diagnostics | redaction or denial | no actionability | observer cannot mutate |
| `developer_break_glass` | explicit debug path | audit log | diagnostic-only | logged failure or explicit allow | no production projection delta | audit/log assertion |

## 4. Required Behavior

### 4.1 Token resolution

Every principal must resolve to exactly one caller class before transport dispatch.

Ambiguous, missing, or compatibility tokens must fail closed unless a test-only fixture explicitly opts into compatibility behavior.

### 4.2 GraphQL

GraphQL must:

- deny non-UI callers unless provisioned as read-only observers;
- deny all non-approval mutations for `ui_operator`;
- deny approval mutations when the approval is not actionable for that principal;
- expose read-only actionability and disabled-reason fields that match auth decisions.

### 4.3 MCP

MCP must:

- be the external command/control surface for agents and automations;
- reject commands outside the caller capability set before command journal mutation;
- return stable denial reason codes;
- never rely on GraphQL actionability to authorize MCP commands.

### 4.4 Approvals

Approval actionability must be derived from durable approval state and caller class. Caller-supplied provenance may be diagnostic input only; it must not become authority.

## 5. Tests

Add one focused proof gate:

```text
proposal-081|p081
```

Minimum test coverage:

- token resolution maps every fixture principal to one caller class;
- GraphQL query allow/deny for `ui_operator`, `agent_operator`, `automation`, and `observer`;
- GraphQL mutation allow list contains only `approveApproval` and `rejectApproval` for UI;
- MCP command allow/deny matrix for agent and automation principals;
- approval actionability fields match mutation authorization;
- denied calls do not write command journal, approval settlement, or projection deltas.

## 6. Non-Goals

- Do not add new UI write behavior.
- Do not remove compatibility fixtures without a migration plan.
- Do not move UI control into MCP.
- Do not make GraphQL the agent control plane.

## 7. Acceptance Criteria

P081 is complete when:

1. the boundary matrix exists in `docs/reference/`;
2. auth, GraphQL, MCP, and approval actionability tests cite the matrix rows they prove;
3. PR #3 boundary findings have an explicit row, test, or out-of-scope disposition;
4. future UI-write proposals must update the matrix before code changes.
