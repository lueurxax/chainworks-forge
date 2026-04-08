# Proposal 029: Second-Wave ACP Runtime Profiles — Codex, Auggie, Junie

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Codex / Andrey Khasanov |
| Depends on | [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Expand the ACP runtime with three second-wave providers (Codex ACP, Auggie CLI ACP, Junie CLI ACP), including the required provider-platform expansion, fail-closed transport factory, MCP namespace migration, and capability enforcement ownership. |
| Goal | Add three real ACP runtimes to the catalog while preserving Forge execution truth and closing the structural gaps identified in the R1 review. |

---

## 1. Context and Motivation

The current first-wave ACP baseline established:

- ACP-shaped core transport vocabulary (`RuntimeTransportProtocol`)
- Goose preserved as default continuity path
- Two first-wave ACP adapters: `claude_agent_acp` and `gemini_cli_acp`
- Runtime profile and backend profile selection through catalog data

Research has advanced enough to justify a second expansion wave: Codex ACP, Auggie CLI ACP, and Junie CLI ACP.

However, the R1 review of this proposal identified four structural problems that must be resolved before implementation:

1. **The proposal claimed to be "catalog data only" but actually requires a provider-platform expansion.** Current `ProviderFamily` only supports `codex`, `claude`, `gemini`. New providers need new families, seeded settings, health adapters, and setup/readiness ownership.

2. **The staged rollout is unsafe.** The transport factory (`DefaultRuntimeTransportFactory`) silently falls back to Goose for unknown `adapterFamily` values. Shipping profile scaffolding before adapters would create runs that persist ACP intent but execute on Goose — a truth corruption.

3. **MCP namespace ownership is Goose-only.** `MCPPolicyResolver.runtimeNamespace(for:)` returns `nil` for non-Goose bindings. First-wave ACP already proved namespace-specific behavior for `claude_agent` and `gemini_cli`, so second-wave providers need the same explicit namespace mapping.

4. **New capability tokens have no enforcement consumer.** The proposal introduced `session_new`, `session_load`, `mcp_attach`, etc. but nothing in the codebase reads or enforces `requires` on `RuntimeProfile`.

This revision addresses all four.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can catalog/runtime-profile selection support three additional ACP runtimes without changing the core transport seam?
2. Does the transport factory **refuse** to create a transport for an unregistered adapter family instead of silently falling back to Goose?
3. Does each second-wave provider have an explicit MCP runtime namespace so preflight can reconcile extensions correctly?
4. Is the `requires` capability vocabulary on `RuntimeProfile` enforced by a locked consumer before any run starts?
5. Does the provider-platform expansion (new `ProviderFamily` cases, adapters, seeded settings) remain additive and non-breaking for existing Goose/Claude/Gemini workflows?

---

## 3. Scope

This proposal includes:

- **Provider-platform expansion**: new `ProviderFamily` cases (`.codexACP`, `.auggie`, `.junie`), corresponding `ProviderAdapter` implementations, seeded settings, and setup/readiness ownership
- **Fail-closed transport factory**: change `RuntimeTransportFactory` protocol to `throws`, add `RuntimeTransportError.unknownAdapterFamily`, and update the full owner chain (factory → executor → preflight → report)
- **Three ACP adapter stubs**: `CodexACPTransport`, `AuggieCLIACPTransport`, `JunieCLIACPTransport`
- **Runtime profile definitions**: `codex_acp`, `auggie_cli_acp`, `junie_cli_acp` in catalog YAML
- **Backend profile variants**: targeting each runtime profile with correct provider/model bindings
- **MCP registry migration**: promote `GooseExtensionRegistrySnapshot` / `GooseExtensionRegistryReader` to transport-neutral `RuntimeExtensionRegistry` with per-adapter install/readiness owners, and update `MCPPolicyResolver` to resolve against the registry for the binding's adapter family instead of hard-coding Goose
- **Capability enforcement via existing `ProviderCapabilities`**: extend `ProviderCapabilities` (not a parallel authority) with runtime-profile-derived fields, and add preflight validation that checks adapter-declared capabilities against `RuntimeProfile.requires`
- **Rollout gating**: per-adapter feature flag on `ConfiguredProvider.isEnabled` — single owner for enablement, no parallel enablement lanes

This proposal does **not** include:

- Changing Goose default status or removing it as fallback for `adapterFamily == "goose"`
- Hard runtime cutover
- Asserting operator-grade classification for any second-wave provider
- Cross-provider MCP parity claims

---

## 4. Design

### 4.1 Provider-Platform Expansion

**Current state**: `ProviderFamily` is `codex | claude | gemini`. Each has a `ProviderAdapter`, seeded `ConfiguredProvider`, health probe, and readiness flow.

**Change**: Add three new families:

```swift
enum ProviderFamily: String, Codable, CaseIterable, Sendable {
    case codex
    case claude
    case gemini
    case codexACP    // Codex via ACP stdio, distinct from Goose-backed .codex
    case auggie
    case junie
}
```

Each new family gets:
- `runtimeProviderIdentifier` mapping (e.g., `.codexACP` → `"codex_acp"`)
- `ProviderAdapter` implementation with health probe
- Seeded `ConfiguredProvider` entry (disabled by default, operator enables via Settings)
- Display name and model validation in `isValidModel(identifier:)`

**Why `.codexACP` instead of reusing `.codex`?** The existing `.codex` family maps to the Goose-backed Codex transport. Codex ACP uses a fundamentally different transport (ACP stdio, not Goose REST/SSE). Sharing a family would conflate two transport paths in preflight, report, and recovery surfaces. Using `.codexACP` makes the transport distinction explicit in the type system.

### 4.2 Fail-Closed Transport Factory

**Current state**: `DefaultRuntimeTransportFactory` (`ExecutionService.swift:916`) explicitly handles `claude_agent_acp` and `gemini_cli_acp`. Unknown families silently fall back to Goose. The current ACP transport baseline already exercises canonical ACP proof flows (proposal loop + implementation path), confirming the factory works for registered families. But the fallback behavior remains unsafe for second-wave rollout.

The safety fix requires changes across the full owner chain, not just the factory's `default` case.

**Step 1 — Protocol change** (`RuntimeTransport.swift:210`):

Current: `func transport(for:binding:) -> any RuntimeTransportProtocol` (non-throwing)

Change to: `func transport(for:binding:) throws -> any RuntimeTransportProtocol`

This is a breaking protocol change. All conformers must be updated:
- `DefaultRuntimeTransportFactory` (the real factory)
- `SingleTransportFactory` (test/backward-compat shim)
- Any test mocks

**Step 2 — New error case** (`RuntimeTransport.swift:183`):

Add to `RuntimeTransportError`:
```swift
case unknownAdapterFamily(String)
```

**Step 3 — Factory implementation** (`ExecutionService.swift:927`):

```swift
default:
    throw RuntimeTransportError.unknownAdapterFamily(family)
```

**Step 4 — Executor call site** (`GooseAgentExecutor` or `RuntimeAgentExecutor`):

The executor calls `factory.transport(for:binding:)`. Since this now throws, the executor must catch `unknownAdapterFamily` and surface it as an agent-level failure with a clear error message, not a `fatalError`.

**Step 5 — Preflight integration** (`PreflightService`):

Preflight should validate adapter family registration **before run start** by attempting `factory.transport(for:binding:)` in dry-run mode, or by checking a registered-families set. This catches the error at preflight time (operator-visible) rather than mid-execution.

**Migration safety**: The existing `"goose"` case remains explicit (not default), so current workflows are unaffected. Tests using `SingleTransportFactory` get a trivial `throws` annotation with no behavior change.

### 4.3 MCP Registry Migration

**Current state**:

Namespace resolution is already transport-neutral. `ResolvedProviderBinding.effectiveRuntimeNamespace` (`BackendProfileResolverV2.swift:49`) switches on `adapterFamily` and already handles `claude_agent_acp`, `gemini_cli_acp`, and `codex_acp`. `MCPPolicyResolver.runtimeNamespace(for:)` delegates to this property.

However, the registry layer beneath namespace resolution is still Goose-owned:
- `resolveServer()` takes `gooseRegistry: GooseExtensionRegistrySnapshot?` (`MCPPolicyRuntime.swift:304`)
- `GooseExtensionRegistryReader` is still the only `RuntimeExtensionRegistryProvider` conformer
- Preflight loads the registry as "Goose Extension Registry" (`PreflightService.swift`)

This proposal needs to fix the registry layer, not the namespace layer.

**Step 1 — Extend existing namespace switch**:

Add `auggie_cli_acp` and `junie_cli_acp` cases to `effectiveRuntimeNamespace`:

```swift
case "auggie_cli_acp": return "auggie"
case "junie_cli_acp":  return "junie"
```

**Step 2 — Transport-neutral registry abstraction**:

Rename `GooseExtensionRegistrySnapshot` → `RuntimeExtensionRegistrySnapshot`. `GooseExtensionRegistryReader` remains as the Goose-specific `RuntimeExtensionRegistryProvider` conformer. Add new conformers per second-wave adapter family.

**Step 3 — Registry resolution by adapter family**:

Change `resolveServer()` from `gooseRegistry: GooseExtensionRegistrySnapshot?` to `registrySnapshot: RuntimeExtensionRegistrySnapshot?`. The caller resolves the correct snapshot for the binding's adapter family.

**Step 4 — Preflight integration**:

Preflight resolves the registry provider per binding. If no registry exists for an adapter family, MCP-dependent agents are blocked.

**Migration safety**: Goose path unchanged — `GooseExtensionRegistryReader` remains the conformer for `adapterFamily == "goose"`.

### 4.4 Capability Enforcement

**Current state**: Operator-visible capability truth already lives on `ConfiguredProvider.capabilities` (`ProviderCapabilities` struct with `supportsStreaming`, `supportsTools`, `supportsStructuredOutput`, `supportsEffortControl`, `supportsSessionResume`, `supportsFileEditing`, `supportsSandboxHints`). This is the existing single authority for what a provider can do.

Meanwhile, `RuntimeProfile.requires` is `[String]` stored in catalog but never read by any enforcement path — it's a dangling second authority.

**Design decision**: Runtime-profile capabilities **extend the existing `ProviderCapabilities` owner**, not create a parallel one. The `requires` vocabulary on `RuntimeProfile` maps directly to `ProviderCapabilities` fields.

**Step 1 — Map `requires` tokens to `ProviderCapabilities` fields**:

| `requires` token | `ProviderCapabilities` field | Already exists? |
|------------------|------------------------------|-----------------|
| `streaming` | `supportsStreaming` | Yes |
| `tools` | `supportsTools` | Yes |
| `structured_output` | `supportsStructuredOutput` | Yes |
| `effort_control` | `supportsEffortControl` | Yes |
| `session_resume` | `supportsSessionResume` | Yes |
| `file_editing` | `supportsFileEditing` | Yes |
| `mcp_reconciliation` | `supportsMCPReconciliation` | **New** — add to `ProviderCapabilities` |

**Step 2 — Seeded capabilities per new family**:

`ProviderCapabilities.default(for:)` already switches on `ProviderFamily`. Add cases for `.codexACP`, `.auggie`, `.junie` with their known capability truth from probe evidence.

**Step 3 — Preflight validation**:

Preflight reads `RuntimeProfile.requires`, maps each token to the corresponding `ProviderCapabilities` field on the binding's `ConfiguredProvider`, and blocks on mismatch:

```swift
for (agentID, binding) in bindings {
    guard let profile = runtimeProfiles[binding.runtimeProfileID] else { continue }
    let capabilities = configuredProvider(for: binding).capabilities
    let unsatisfied = profile.requires.filter { !capabilities.satisfies($0) }
    if !unsatisfied.isEmpty {
        issues.append(.capabilityMismatch(agentID: agentID, unsatisfied: unsatisfied))
    }
}
```

**Removed tokens**: `session_new`, `session_load`, `session_update`, `tool_call_visibility`, `mcp_attach`, `session_persistence` are removed from this proposal. They either map to existing fields (`session_resume`) or have no locked consumer. The current transport contract has no explicit load/update capability surface, so `session_persistence` would be speculative.

### 4.5 Candidate Mapping

| Provider | Adapter Family | Capability Class | MCP Namespace | Requires |
|----------|---------------|------------------|---------------|----------|
| Codex ACP | `codex_acp` | `control_capable` | `codex` | `streaming`, `tools`, `session_resume` |
| Auggie CLI ACP | `auggie_cli_acp` | `control_capable` | `auggie` | `streaming`, `tools` |
| Junie CLI ACP | `junie_cli_acp` | `control_capable` | `junie` | `streaming`, `tools` |

### 4.6 Catalog Configuration

```yaml
runtime_profiles:
  codex_acp:
    capability_class: control_capable
    adapter_family: codex_acp
    requires:
      - streaming
      - tools
      - session_resume
    transport_kind: acp_stdio
    mcp_realization_path: acp_native

  auggie_cli_acp:
    capability_class: control_capable
    adapter_family: auggie_cli_acp
    requires:
      - streaming
      - tools
    transport_kind: acp_stdio
    mcp_realization_path: acp_native

  junie_cli_acp:
    capability_class: control_capable
    adapter_family: junie_cli_acp
    requires:
      - streaming
      - tools
    transport_kind: acp_stdio
    mcp_realization_path: acp_native

backend_profiles:
  codex_orchestrator_acp:
    provider: codex_acp
    model: gpt-5
    effort: medium
    runtime_profile: codex_acp

  auggie_orchestrator_acp:
    provider: auggie
    model: auggie-default
    effort: medium
    runtime_profile: auggie_cli_acp

  junie_orchestrator_acp:
    provider: junie
    model: junie-default
    effort: medium
    runtime_profile: junie_cli_acp
```

### 4.7 Rollout Order

1. **Phase 1 — Structural prerequisites** (this proposal):
   - Fail-closed transport factory
   - MCP namespace migration for all adapter families
   - Capability enforcement in preflight
   - Provider-platform expansion (new families, adapters, seeded settings)

2. **Phase 2 — Codex ACP adapter**:
   - Register `CodexACPTransport` in transport factory
   - Declare capabilities
   - Run proof gate

3. **Phase 3 — Auggie + Junie adapters**:
   - Register `AuggieCLIACPTransport` and `JunieCLIACPTransport`
   - Run proof gates

**Critical invariant**: Phase 1 ships the fail-closed factory **before** any profile scaffolding. No profile can exist in catalog data until its adapter is registered, because the factory will refuse to create the transport.

### 4.8 Disabled-Provider Rollout Semantics

**Current state**: `ConfiguredProvider` has no `isEnabled` field. `ProviderRegistry.preferredProvider(for:)` returns the preferred or first provider matching a family — no filtering. `ProviderSettingsStore.seededDefault()` seeds all families unconditionally. `removeProvider(id:)` repairs `preferredProviderIDsByFamily` to the next available same-family instance.

**Change**: Add `isEnabled: Bool` to `ConfiguredProvider` (default `true` for existing families, `false` for second-wave families). This is the single rollout gate — no parallel feature flags.

**Integration points**:

1. **`ProviderRegistry.preferredProvider(for:)`**: Filter to `isEnabled == true` before selecting. If no enabled provider exists for a family, return `nil`. This makes the family effectively unavailable without removing the `ConfiguredProvider` instance.

2. **`ProviderSettingsStore.seededDefault()`**: Seed second-wave providers with `isEnabled: false`. They appear in Settings but cannot be selected for runs until the operator enables them.

3. **`BackendProfileResolverV2.resolveBindings()`**: When resolving a binding whose `ProviderFamily` has no enabled `ConfiguredProvider`, fail with a clear error ("Provider family .codexACP is configured but not enabled").

4. **Preflight**: Check `isEnabled` before capability validation. A disabled provider is not a capability mismatch — it's a rollout gate. Surface as "Provider not enabled" (actionable), not "Capability unsatisfied" (confusing).

5. **Settings UI**: Show disabled providers with a toggle. Operator enables when ready. No separate feature-flag UI.

6. **Preferred-provider repair**: `removeProvider(id:)` and `setPreferredProvider(id:for:)` already repair `preferredProviderIDsByFamily`. Add: when the preferred provider becomes disabled, repair to the next enabled same-family instance (or clear if none).

7. **Diagnostics/Report**: `isEnabled` state is included in preflight readiness summary so operators can see which providers are available vs configured-but-disabled.

**Migration safety**: Existing families (`.codex`, `.claude`, `.gemini`) get `isEnabled: true` by default — no behavior change. Only new second-wave families start disabled.

---

## 5. Acceptance Criteria

1. Three new `ProviderFamily` cases (`.codexACP`, `.auggie`, `.junie`) exist with adapters, seeded `ProviderCapabilities`, and health probes.
2. `RuntimeTransportFactory.transport(for:binding:)` is `throws`. `DefaultRuntimeTransportFactory` throws `RuntimeTransportError.unknownAdapterFamily` for unregistered families. The executor catches this and surfaces it as an agent-level failure. Preflight validates adapter registration before run start.
3. MCP registry is transport-neutral: `GooseExtensionRegistrySnapshot` is renamed to `RuntimeExtensionRegistrySnapshot`, `MCPPolicyResolver` resolves against the correct adapter's registry provider, and each ACP adapter family has an explicit MCP namespace.
4. Preflight validates `RuntimeProfile.requires` against `ProviderCapabilities` fields on `ConfiguredProvider` — no parallel capability authority.
5. Every `requires` token maps to an existing or newly-added `ProviderCapabilities` field with a locked consumer.
6. Default Goose path remains operational.
7. Run snapshots and execution reports preserve truth consistently across all provider families.
8. Rollout enablement uses `ConfiguredProvider.isEnabled` as single owner: `preferredProvider(for:)` filters by `isEnabled`, preflight distinguishes "not enabled" from "capability mismatch", and preferred-provider repair respects disabled state. Second-wave families seed as disabled by default.
9. A focused `proposal-029` gate in `test-gate.sh` passes on the canonical tree.

---

## 6. Risks

- **Capability regression**: a provider's behavior changes after onboarding and weakens proof quality.
- **Capability inflation**: treating second-wave providers as operator-grade before evidence grows.
- **Onboarding burden**: provider-specific auth/config steps increase operator setup complexity.
- **Provider-family proliferation**: five families (plus three new) may strain the settings/readiness UI. Mitigated by shipping new families disabled by default.

---

## 7. Alternatives Considered

### 7.1 Catalog-only expansion without provider-platform changes

Rejected by R1 review. Backend profiles for second-wave providers require new `ProviderFamily` cases (`.codexACP`, `.auggie`, `.junie`), adapters, and seeded settings. Claiming "catalog data only" while requiring platform expansion is a scope mismatch.

### 7.2 Reuse existing ProviderFamily cases

Rejected. `.codex` maps to Goose-backed Codex. Sharing it with Codex ACP would conflate two transport paths in preflight, report, and recovery. `.codexACP` is the correct family.

### 7.3 Ship profile scaffolding before adapters (original staged rollout)

Rejected by R1 review. The transport factory's Goose fallback would silently execute ACP-intended runs on Goose — a truth corruption. The fail-closed factory must ship first.

### 7.4 Keep speculative capability tokens

Rejected by R1 review. `session_new`, `session_load`, `tool_call_visibility`, etc. had no enforcement consumer. This revision limits the vocabulary to tokens with locked consumers.
