# UI Action Boundary

Stable reference for the governed macOS UI action boundary.

This document is the single short source of truth for which operator actions may
be invoked from SwiftUI, GraphQL, MCP, agents, and automation.

## Core Rule

The governed SwiftUI app is a GraphQL-only observer and approval console.

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

## Enforcement (P073)

The UI action boundary is enforced by the control plane's authentication layer.

- **`forge-app-graphql` principal**: The default principal for governed-app GraphQL read and subscription traffic. It is restricted to the `app_graphql_readonly` profile and allows zero mutations.
- **`default-operator` principal**: The primary app bearer, restricted to exactly the `approveApproval` and `rejectApproval` mutations on GraphQL by default.
- **`graphql-break-glass-operator`**: A manually activated compatibility principal that allows full GraphQL write access for break-glass recovery or testing.

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
- Historical thin UI proposal: [../proposals/031-thin-graphql-ui-rewrite.md](../proposals/031-thin-graphql-ui-rewrite.md)
