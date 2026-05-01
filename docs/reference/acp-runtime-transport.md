# ACP Runtime Transport

Stable reference for the ACP-only runtime transport layer in Chainworks Forge, including runtime selection, adapter families, and persisted runtime truth.

## Purpose

Runtime execution should be transport-neutral at the control-plane level.

The app must be able to:

- compile runs without binding orchestration to provider-specific endpoint semantics,
- select a runtime through catalog/runtime-profile truth,
- persist transport-neutral execution truth,
- support multiple ACP-capable runtimes,
- and keep the control plane independent from any one adapter family.

## Scope

This reference covers:

- `RuntimeTransportProtocol`,
- runtime transport factory selection,
- catalog-owned runtime profiles,
- backend-profile to runtime-profile binding,
- ACP adapter families,
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

## Canonical transport contract

Core execution depends on `RuntimeTransportProtocol` as the stable ACP runtime boundary.

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
| `codex_acp` | `codex_acp` | `operator_grade` | `acp_stdio` | `acp_native` |
| `auggie_cli_acp` | `auggie_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |
| `junie_cli_acp` | `junie_cli_acp` | `control_capable` | `acp_stdio` | `acp_native` |

`RuntimeProfile.requires` is a normative capability map — it gates launch, startup preflight, and MCP-policy reconciliation. Profiles that are known but missing required capabilities block run execution via the deterministic readiness contract. Disabled, configured, and unavailable states remain distinct in operator-facing surfaces (not collapsed into a single message path).

## Backend-profile ownership

Agents continue to select only `backend_profile`.

`backend_profile` remains the single repo-owned bundle for:

- provider family,
- model,
- effort,
- structured-output intent,
- and backend-owned required MCP.

This keeps runtime selection attached to the same binding lane that already owns provider/model intent.

In the current catalog:

- Claude-backed operator and writer profiles bind to `claude_agent_acp`
- Gemini review profiles bind to `gemini_cli_acp`
- Codex-backed implementation and authoring profiles bind to `codex_acp`
- Auggie and Junie remain ACP-only families where configured

The current resolver path is:

```text
backend_profile
  -> optional runtime_profile
  -> BackendProfileResolverV2
  -> ResolvedProviderBinding(runtimeProfileID, adapterFamily, capabilityClass)
  -> RuntimeTransportFactory
```

## Implemented transport families

### ACP-native adapters

The currently implemented ACP-native adapters are normalized into canonical 
provider families (`claude`, `gemini`, `codex`, `auggie`, `junie`) for 
consistent capacity management and backpressure:

- `ClaudeAgentACPTransport` (canonical: `claude`)
- `GeminiCLIACPTransport` (canonical: `gemini`)
- `CodexACPTransport` (canonical: `codex`)
- `AuggieCLIACPTransport` (canonical: `auggie`)
- `JunieCLIACPTransport` (canonical: `junie`)

Shared ACP plumbing lives in:

- `ACPSubprocessManager`
- `ACPStreamEventMapper`

#### Toolchain Cache Mapping

The ACP layer manages the isolation of toolchain-specific build and cache roots, including Xcode DerivedData and Go caches. This section is the stable owner for provider-launched toolchain cache mapping behavior.

- **Environment Redirection**: ACP adapters derive the appropriate toolchain root based on the agent's `toolchain_cache_policy` and session/run scope. They publish `CHAINWORKS_TOOLCHAIN_HOME` and `TOOLCHAIN_HOME` and apply family-specific redirection (e.g., `-derivedDataPath` for Xcode, `GOCACHE` for Go).
- **Exclusive Serialization**: For run-scoped Xcode work, the host-executor path acquires an exclusive per-run lease to prevent concurrent mutation of the same DerivedData root.
- **Diagnostics**: Adapters capture setup and mapping metadata, stored as `actualToolchainMappingDiagnostics` on the execution record.
- **Apple Read Adapter**: Swift operator-facing consumers decode toolchain mapping truth through `ToolchainMappingReadAdapter` to ensure consistent handling of frozen-snapshot compatibility and legacy sentinels.

#### Bounded Discovery and DiscoveryFilesystem

Broad filesystem discovery is not part of the pre-`initialize` path. Instead of implicit inference from the entire repository or worktree, the system uses a bounded discovery model:

- **DiscoveryFilesystem Ownership**: Shared discovery value-types and filesystem logic live in `domain::discovery`, while policy construction remains engine-owned.
- **Bounded Discovery**: Discovery is restricted to the run meta-root and explicitly declared expected output paths.
- **Pre-Prompt Metadata**: Metadata capture is now a per-execution, per-prompt-turn step for both fresh and reused sessions.
- **Settlement Pipeline**: An engine-owned pipeline settles discovered artifacts based on typed expected outputs and discovery decisions.

## Current factory behavior

`DefaultRuntimeTransportFactory` now resolves only ACP families:

1. `adapterFamily == "claude_agent_acp"` -> `ClaudeAgentACPTransport`
2. `adapterFamily == "gemini_cli_acp"` -> `GeminiCLIACPTransport`
3. `adapterFamily == "codex_acp"` -> `CodexACPTransport`
4. `adapterFamily == "auggie_cli_acp"` -> `AuggieCLIACPTransport`
5. `adapterFamily == "junie_cli_acp"` -> `JunieCLIACPTransport`

Unknown families fail closed.

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

The implemented ACP baseline currently guarantees:

1. core execution code depends on `RuntimeTransportProtocol` as the canonical ACP transport boundary,
2. runtime profile choice is frozen into run-start binding truth,
3. operator/report/recovery surfaces read persisted Forge truth rather than adapter-local heuristics,
4. unknown adapter families fail closed,
5. repo-owned catalog data can target Claude, Gemini, Codex, Auggie, and Junie ACP families.

## Current implementation owners

- `Chainworks Forge/Engine/RuntimeTransport.swift`
- `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
- `Chainworks Forge/Engine/ExecutionService.swift`
- `Chainworks Forge/Engine/RuntimeSessionBridge.swift`
- `Chainworks Forge/Engine/RuntimeAgentExecutor.swift`
- `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/GeminiCLIACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift`
- `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift`
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
