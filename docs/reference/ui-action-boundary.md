# UI Action Boundary

Stable reference for the governed macOS UI action boundary.

This document is the single short source of truth for which operator actions may
be invoked from SwiftUI, GraphQL, MCP, agents, and automation.

## Core Rule

The governed SwiftUI app is a GraphQL-only observer and approval console.
Non-approval GraphQL mutations are prohibited from governed UI code.

SwiftUI may use:

- GraphQL read queries,
- GraphQL subscriptions,
- the `approveApproval` GraphQL mutation,
- the `rejectApproval` GraphQL mutation.

SwiftUI must not use:

- MCP tools,
- direct SQLite access,
- local workflow mutation fallback,
- Swift-local workflow truth,
- broad GraphQL command mutations.
- escalation policy drift acknowledgement.
- tier mutation (retry, resume, cancel, or force-primary).

## Forbidden UI Mutations

The governed SwiftUI app must not create, start, cancel, retry, reset, compact,
clone, recover, mutate runtime profiles, mutate context strategy, run
experiments, or perform other operational commands.

Forbidden GraphQL mutation families for governed SwiftUI include:

- create / start,
- cancel,
- retry,
- reset,
- compact,
- clone / fork,
- runtime profile changes,
- context strategy changes,
- experiments,
- recovery / repair.
- escalation (acknowledgement, tier mutation, etc.).

## MCP Boundary

MCP owns all non-approval operator operations.

Examples:

- `ideas.create`,
- `runs.start`,
- `runs.cancel`,
- `stages.retry`,
- `workflow_conflicts.resolve`,
- `legacy_discovery_override_create`,
- `steward.run_analysis`,
- reset / compact / clone / recover operations when present.

MCP may also expose reads and reports for agents, CLI users, automations, and
diagnostic tools. That does not make MCP part of the governed SwiftUI surface.

## Agent And Automation Boundary

Agents and automations must use MCP/control-plane tools for operational actions.
They must not use GraphQL mutations or direct SQLite writes as a control path.

GraphQL is for UI reads, UI subscriptions, and the two approval mutations only.
SQLite is internal daemon storage, not an automation API.

## Owner References

- GraphQL projection read shape: [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md)
- MCP command/control surface: [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md)
- Operator shell behavior: [operator-experience.md](operator-experience.md)
