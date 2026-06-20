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
- broad GraphQL command mutations,
- escalation policy drift acknowledgement,
- tier mutation (retry, resume, cancel, or force-primary).

## Local Enforcement

The Swift app has a local boundary guard in addition to server authorization. `P031GraphQLReadRequest` rejects non-query/non-subscription documents for the read client and rejects forbidden mutation operation names. `P072ApprovalMutationClient` is the narrow exception: it sends only the `approveApproval` and `rejectApproval` documents through the approval mutation path.

The app also resolves the daemon endpoint from the packaged endpoint/port files and reads daemon/storage diagnostics through GraphQL. Endpoint discovery and lifecycle banners are presentation/readback code; they do not create a fallback mutation channel.

## Boundary Matrix

| Row ID | SwiftUI surface | Allowed transport | Allowed action | Denied action families |
|---|---|---|---|---|
| `P081-UI-APPROVAL-APPROVE` | Approval inbox / approval detail | GraphQL mutation | `approveApproval` only when durable approval state is pending/requested and caller policy allows it | start/cancel/retry/reset/compact/clone/recovery/runtime-profile/context-strategy/experiment |
| `P081-UI-APPROVAL-REJECT` | Approval inbox / approval detail | GraphQL mutation | `rejectApproval` only when durable approval state is pending/requested and caller policy allows it | start/cancel/retry/reset/compact/clone/recovery/runtime-profile/context-strategy/experiment |
| `P081-UI-READ-ONLY` | Runs, stages, artifacts, diagnostics, freshness badges | GraphQL query/subscription | read-only projection display | all mutations and all direct SQLite/local workflow writes |
| `P081-UI-EXTERNAL-COMMANDS` | Command placeholders and guidance | MCP/control-plane outside governed SwiftUI | none from SwiftUI | all non-approval command/control operations from GraphQL or local Swift state |

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
- recovery / repair,
- escalation acknowledgement or tier mutation.

## MCP Boundary

MCP owns all non-approval operator operations.

Examples:

- `ideas.create`,
- `runs.start`,
- `runs.cancel`,
- `stages.retry`,
- `effects.list`,
- `effects.inspect`,
- `effects.reconcile`,
- `effects.mark_conflict`,
- `effects.mark_unrecoverable`,
- `effects.clear_after_manual_verification`,
- `storage.maintenance.repair_slot`,
- `storage.projections.clear_backlog`,
- `storage.projections.clear_poison`,
- `agents.continue_work`,
- `workflow_conflicts.resolve`,
- `workflow_loop_budget.extend`,
- `runs.retrofit_catalog_snapshot`,
- `legacy_discovery_override_create`,
- `steward.run_analysis`,
- reset / compact / clone / recover operations when present.

MCP may also expose reads and reports for agents, CLI users, automations, and
diagnostic tools. That does not make MCP part of the governed SwiftUI surface.

## Agent And Automation Boundary

Agents and automations must use MCP/control-plane tools for operational actions.
They must not use GraphQL mutations or direct SQLite writes as a control path.

For governed SwiftUI, GraphQL is for UI reads, UI subscriptions, and the two
approval mutations only. The P083 operator GraphQL lifecycle mutations
(`providerSessionShutdown`, `p083MarkProviderSessionProcessAbsent`,
`p083RollbackExecution`, `p083SetEnforcementMode`, `runsRetry`) are
non-UI command surface for explicitly authorized operator callers and
are not accessible from governed SwiftUI. SQLite is internal daemon
storage, not an automation API.

## Owner References

- GraphQL projection read shape: [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md)
- Thin-client affordance contract: [thin-client-read-model-affordance-contract.md](thin-client-read-model-affordance-contract.md)
- MCP command/control surface: [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md)
- Operator shell behavior: [operator-experience.md](operator-experience.md)
