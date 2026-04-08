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
- catalog-owned runtime profiles,
- backend-profile to runtime-profile binding,
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

## Catalog-owned runtime profiles

Runtime profile intent is stored in the agent catalog, not invented in view code or machine-local settings.

Current owner fields live in `AgentCatalog.RuntimeProfile`:

- `capability_class`
- `adapter_family`
- `requires`
- `transport_kind`
- `mcp_realization_path`

Current repo-backed runtime profiles are:

| Runtime profile | Adapter family | Capability class | Transport kind | MCP realization |
|---|---|---|---|---|
| `claude_agent_acp` | `claude_agent_acp` | `operator_grade` | `acp_stdio` | `acp_native` |
| `gemini_cli_acp` | `gemini_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |

The catalog currently does not ship second-wave runtime profiles such as Codex ACP, Auggie CLI ACP, or Junie CLI ACP.

## Backend-profile ownership

Agents continue to select only `backend_profile`.

`backend_profile` remains the single repo-owned bundle for:

- provider family,
- model,
- effort,
- structured-output intent,
- and optional `runtime_profile`.

This keeps runtime selection attached to the same binding lane that already owns provider/model intent.

In the current catalog:

- Claude-backed operator and writer profiles bind to `claude_agent_acp`
- Gemini review profiles bind to `gemini_cli_acp`
- backend profiles without `runtime_profile` continue to execute through the Goose compatibility path

The current resolver path is:

```text
backend_profile
  -> optional runtime_profile
  -> BackendProfileResolverV2
  -> ResolvedProviderBinding(runtimeProfileID, adapterFamily, capabilityClass)
  -> RuntimeTransportFactory
```

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

## Current factory behavior

`DefaultRuntimeTransportFactory` currently has three effective paths:

1. `adapterFamily == "goose"` or missing runtime profile -> shared Goose transport
2. `adapterFamily == "claude_agent_acp"` -> `ClaudeAgentACPTransport`
3. `adapterFamily == "gemini_cli_acp"` -> `GeminiCLIACPTransport`

Important current limitation:

- unknown non-Goose adapter families still fall back to Goose when Goose is configured

That fallback is the current implementation truth and should be treated as a compatibility behavior, not as a future-safe rollout contract.

## Persisted runtime truth

The runtime slice is only useful if transport decisions become durable execution truth.

Current persisted lanes include:

- run-start provider/runtime binding snapshot,
- runtime-profile and adapter-family truth on execution records,
- effective runtime namespace used for MCP/runtime settlement,
- report/comparison visibility of actual runtime family used,
- recovery logic that reasons from frozen runtime truth instead of current disk defaults.

This is what keeps an ACP-backed run explainable after relaunch or resume.

Concrete persisted execution truth includes:

- `ResolvedProviderBinding.runtimeProfileID`
- `ResolvedProviderBinding.adapterFamily`
- `ResolvedProviderBinding.capabilityClass`
- `AgentExecution.runtimeProfileID`
- `AgentExecution.actualAdapterFamily`

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

## Current invariants

The implemented first-wave ACP baseline currently guarantees:

1. core execution code depends on `RuntimeTransportProtocol`, not Goose endpoint semantics,
2. Goose remains the default continuity path for backend profiles without explicit runtime profiles,
3. runtime profile choice is frozen into run-start binding truth,
4. operator/report/recovery surfaces read persisted Forge truth rather than adapter-local heuristics,
5. Claude Agent ACP and Gemini CLI ACP are the only first-wave ACP adapters currently supported in repo-owned catalog data.

The implementation does **not** currently guarantee:

- fail-closed rejection for every unknown adapter family,
- second-wave ACP provider support,
- or Goose removal as the default compatibility path.

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

- dedicated ACP transport capability regression coverage on the current tree
- current focused verification summary `71/71` passed
- capability verification includes both canonical ACP-backed proof flows:
  - proposal loop
  - implementation path to manual release gate
- same-tree approved-host `full` green basis:
  - `full-20260408-101540.xcresult`
