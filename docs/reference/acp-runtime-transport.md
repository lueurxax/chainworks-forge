# ACP Runtime Transport

Stable reference for the ACP-shaped runtime transport layer in Chainworks Forge, including runtime selection, adapter families, persisted runtime truth, and Goose's current role as a compatibility adapter inside the transport stack.

## Purpose

Runtime execution should be transport-neutral at the control-plane level.

The app must be able to:

- compile runs without binding core orchestration to Goose endpoint semantics,
- select a runtime through catalog/runtime-profile truth,
- persist transport-neutral execution truth,
- support multiple ACP-capable runtimes,
- and keep Goose available as an adapter without letting it define the core model.

## Scope

This reference covers:

- `RuntimeTransportProtocol`,
- runtime transport factory selection,
- ACP adapter families,
- Goose compatibility adapter role,
- runtime-profile and backend-profile interaction,
- persisted transport-neutral run truth,
- and operator-facing consequences of runtime selection.

It does not define:

- provider credentials and setup UX,
- MCP policy semantics,
- or future second-wave runtime expansion beyond the currently implemented adapters.

## Related docs

- [workflow-execution-engine.md](workflow-execution-engine.md)
- [runtime-contract.md](runtime-contract.md)
- [provider-platform.md](provider-platform.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)
- [live-provider-execution-slice.md](live-provider-execution-slice.md)
- [goose-server-transport.md](goose-server-transport.md)

## Canonical transport contract

Core execution now depends on `RuntimeTransportProtocol`, not on Goose-specific REST or SSE semantics.

The transport contract owns:

- session creation,
- prompt submission,
- stream events,
- session close,
- runtime namespace/capability hints,
- and transport-level diagnostics.

The control plane still owns:

- run lifecycle,
- stage transitions,
- approvals,
- artifact persistence,
- recovery,
- reports,
- and frozen run truth.

## Runtime selection model

Runtime selection is catalog-driven.

The current chain is:

```text
AgentCatalog backend_profile/runtime_profile
  -> BackendProfileResolverV2
  -> ResolvedProviderBinding
  -> RuntimeTransportFactory
  -> selected adapter family
```

No screen or executor should invent a second runtime owner path.

## Implemented transport families

### Goose compatibility adapter

Goose remains implemented and supported through adapter seams:

- `GooseServerTransport`
- `GooseTransport` legacy compatibility path where still applicable
- `FixtureGooseTransport` for deterministic testing

Goose is part of the transport layer, not the canonical control-plane model.

### ACP-native adapters

The currently implemented ACP-native adapters are:

- `ClaudeAgentACPTransport`
- `GeminiCLIACPTransport`

Shared ACP plumbing lives in:

- `ACPSubprocessManager`
- `ACPStreamEventMapper`

## Persisted runtime truth

The runtime slice is only useful if transport decisions become durable execution truth.

Current persisted lanes include:

- run-start provider/runtime binding snapshot,
- runtime-profile and adapter-family truth on execution records,
- report/comparison visibility of actual runtime family used,
- recovery logic that reasons from frozen runtime truth instead of current disk defaults.

This is what keeps an ACP-backed run explainable after relaunch or resume.

## Goose's current role

Goose still matters, but its role is narrower than before:

- local managed runtime continuity,
- compatibility path for existing provider workflows,
- one adapter family within a broader transport system,
- runtime-specific MCP realization where relevant.

What Goose no longer owns:

- the canonical transport vocabulary,
- the control-plane execution model,
- or the only supported live runtime shape.

## ACP operator impact

Operator surfaces continue to read persisted Forge truth rather than adapter internals.

That means:

- reports describe the effective runtime family used,
- comparison can explain runtime-family drift,
- recovery works from frozen transport/runtime truth,
- and runtime selection is visible without exposing raw adapter implementation details.

## Integration with the rest of the app

The transport layer plugs into the app in five stable places:

1. catalog/runtime-profile selection,
2. run-start binding freeze,
3. executor session lifecycle,
4. persisted execution and report truth,
5. operator-facing recovery and comparison.

This is how ACP support became part of the system without rewriting the operator shell.

## Current implementation owners

- `Chainworks Forge/Engine/RuntimeTransport.swift`
- `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
- `Chainworks Forge/Engine/ExecutionService.swift`
- `Chainworks Forge/Engine/GooseSessionBridge.swift`
- `Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/ACPSubprocessManager.swift`
- `Chainworks Forge/Engine/ACPAdapters/ACPStreamEventMapper.swift`
- `Chainworks Forge/Models/AgentExecution.swift`
- `Chainworks Forge/Engine/RunReportBuilder.swift`
- `Chainworks Forge/Engine/RunComparisonService.swift`

## Verification baseline

Current stable verification for this slice is:

- focused same-tree gate:
  - `proposal-026`
  - `71/71` passed on the current tree
- the focused gate includes both canonical ACP-backed proof flows:
  - proposal loop
  - implementation path to manual release gate
- same-tree approved-host `full` green basis:
  - `full-20260408-101540.xcresult`

The historical gate name remains for reproducibility, but the transport layer described here is now permanent reference documentation.
