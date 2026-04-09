# Proposal 034: Clean YAML→Runtime Dispatch by Collapsing Outdated Transport Fragmentation

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Codex |
| Depends on | [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](033-remove-goose-from-canonical-transport-and-simplify-runtime.md), [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md) |
| Scope | Identify and eliminate stale runtime transport selection defaults and duplicate normalization layers between YAML definitions, provider binding resolution, MCP policy, and receipts. |

## 1) Problem statement

Research on the current codebase shows that transport selection has become a blended path:

- legacy app-level metadata (`app.runtime`, `app.transport`) still appears in catalogs,
- provider binding still mixes `transport` and `adapterFamily`,
- runtime-profile resolution injects Goose defaults,
- transport creation still falls back to Goose for missing/unknown families,
- MCP policy and registration still implicitly rely on Goose namespace semantics, and
- provider evidence still carries stale transport assumptions in receipts and UI guidance.

This proposal defines a cleanup lane that keeps ACP as the default canonical execution shape while preserving Goose as an explicit compatibility adapter.

## 2) Current fragment inventory (research-backed)

### A. Defaulting chain still silently biases legacy Goose

- `BackendProfileResolverV2`:
  - if `backendProfile.runtimeProfile` is missing or unresolved, it sets `resolvedAdapterFamily = "goose"` and `resolvedCapabilityClass = .legacyOperatorGrade`.
  - file: [Chainworks Forge/Providers/BackendProfileResolverV2.swift](Chainworks Forge/Providers/BackendProfileResolverV2.swift)
- `DefaultRuntimeTransportFactory`:
  - binds via `binding?.adapterFamily ?? "goose"`,
  - unknown families still fall back to Goose transport.
  - file: [Chainworks Forge/Engine/ExecutionService.swift](Chainworks Forge/Engine/ExecutionService.swift)

Net effect: missing or unknown runtime contracts silently downgrade into Goose instead of failing fast.

### B. Transport inference drifts between layers

- `ResolvedProviderBinding` stores:
  - configured provider transport (`transport`),
  - runtime contract adapter (`adapterFamily`) and optionally capability class.
- `RuntimeTransportFactory` uses `adapterFamily` (ignores configured transport at dispatch time),
- but many code paths still read display/verification hints from provider transport.

This creates a duplicated state model that obscures authoritative runtime intent.

### C. MCP policy is still Goose-centric in selector logic

- `MCPPolicyRuntime.runtimeNamespace(for:)` maps namespace from `providerBinding.transport == goose_server`.
- MCP policy resolution then requires `runtime_ids` mapping keyed by `"goose"` and checks Goose registry snapshot for extension availability.
- files:
  - [Chainworks Forge/Engine/MCPPolicyRuntime.swift](Chainworks Forge/Engine/MCPPolicyRuntime.swift)
  - [Chainworks Forge/Engine/GooseSessionBridge.swift](Chainworks Forge/Engine/GooseSessionBridge.swift)

This makes ACP paths depend on Goose-like metadata or registry behavior in places where they should be explicitly non-blocking.

### D. Multiple normalization layers for same conceptual mapping

Conceptual value gets normalized multiple times:

- configured provider model normalization:
  - [Chainworks Forge/Providers/ConfiguredProvider.swift](Chainworks Forge/Providers/ConfiguredProvider.swift)
- runtime creation transport normalization:
  - [Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift](Chainworks Forge/Engine/GooseAdapter/GooseServerTransport.swift)  
  - [Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift](Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift)
- generated default labels and transport naming still carry Goose-first policy:
  - [Chainworks Forge/Providers/ConfiguredProvider.swift](Chainworks Forge/Providers/ConfiguredProvider.swift)

The result is good to keep compatibility, but hard to audit and error-prone when expanding to new providers.

### E. Evidence surfaces still write legacy transport facts

- `GooseAgentExecutor` writes `transport: "goose"` in all provider receipts:
  - [Chainworks Forge/Engine/GooseAgentExecutor.swift](Chainworks Forge/Engine/GooseAgentExecutor.swift)
- these receipts should reflect resolved runtime execution transport family.

### F. Catalog-level transport metadata has unused/underused fields

- `AgentCatalog.AppConfig` declares `runtime` and `transport`.
- `RuntimeProfile` includes `transport_kind` and `mcp_realization_path`.
- these fields exist but are not consistently used to drive runtime dispatch decisions today.
- files:
  - [Chainworks Forge/DSL/AgentCatalog.swift](Chainworks Forge/DSL/AgentCatalog.swift)
  - examples:
    - [examples/agents/agents.yaml](examples/agents/agents.yaml)
    - [examples/agents/agents_mcp_profiles_v2.yaml](examples/agents/agents_mcp_profiles_v2.yaml)
    - [examples/agents/proposal-po-reviewer.yaml](examples/agents/proposal-po-reviewer.yaml)

## 3) Proposed cleanup plan (no implementation yet)

### 3.1 Make runtime contract single-source and explicit

- Define a single resolved runtime contract emitted by resolver:
  - `runtimeProfileID`, `adapterFamily`, `transportKind`, `mcpRealizationPath`, `requiredCapabilities`, `providerCanonicalModel`, `providerCanonicalIdentifier`.
- Replace implicit `goose` fallback with explicit "no-contract" branch handled by preflight failure or run-start gating.

### 3.2 Drive transport selection from contract, not defaults

- Remove fallback to implicit `"goose"` in:
  - `BackendProfileResolverV2` (missing/invalid runtime profile should be surfaced as hard/clear error),
  - `DefaultRuntimeTransportFactory` (unknown families should fail fast and not assume Goose).
- Keep Goose as a first-class transport family only when the contract resolves to it.

### 3.3 Align MCP policy by runtime mode, not provider transport field

- Replace `transport == goose_server` checks with resolved runtime-mode contract checks.
- For ACP-native modes (`transportKind == acp_*`, `mcpRealizationPath == acp_native`):
  - do not gate on Goose extension registry availability,
  - resolve MCP against ACP-native behavior.
- For Goose mode:
  - continue using Goose extension registry with explicit errors where unavailable.

### 3.4 Collapse model/provider normalization into adapter contracts

- Keep a single adapter-facing normalization surface per runtime kind.
- Remove duplicate implicit conversions scattered between provider defaults, transport constructors, and ACP adapters.
- Preserve compatibility shims in adapter internals, but avoid re-normalizing the same semantic field in higher-level layers.

### 3.5 Propagate resolved runtime transport into receipts and diagnostics

- Replace hardcoded `"goose"` values in usage receipts with resolved runtime contract transport/family identifier.
- Make provider settings/verification messages reflect active, resolved runtime mode.

## 4) Prioritized work packages

1. **Resolver hardening (High)**
   - tighten `BackendProfileResolverV2` fallback semantics,
   - persist explicit runtime contract metadata in `ResolvedProviderBinding`.
2. **Factory correctness (High)**
   - update `DefaultRuntimeTransportFactory` to remove fallback-to-Goose behavior.
3. **MCP policy split-by-mode (High)**
   - update namespace and policy checks in `MCPPolicyRuntime` and `GooseSessionBridge`.
4. **Receipt/telemetry correctness (Medium)**
   - use resolved runtime execution transport in `UsageReceiptNormalizer` call sites.
5. **Normalization de-duplication (Medium)**
   - centralize transport-specific normalization at adapter boundary only.
6. **Catalog hygiene and docs (Medium)**
   - update example files to use explicit runtime-profile-first configuration,
   - document deprecated `app.runtime/app.transport` and stale Goose-first defaults in docs/reference.

## 5) Acceptance criteria

After cleanup:

- No implicit Goose fallback in resolver or transport factory for unresolved/unknown runtime contracts.
- ACP and Goose pathways do not share selector semantics for MCP namespace/registry handling.
- MCP extension resolution is deterministic from resolved runtime contract.
- Receipts and diagnostics expose the real runtime family used in execution.
- At least one end-to-end proof run can pass with:
  - one ACP runtime profile,
  - one CLI runtime profile,
  - one fallback/manual override path.

## 6) Open decisions

- Whether to keep top-level `AppConfig.runtime` / `AppConfig.transport` as informational docs-only fields during transition, or remove them as deprecated and migrate examples in one sweep.
- Whether `legacyOperatorGrade` should remain as a compatibility enum case after full ACP-first baseline proof.
