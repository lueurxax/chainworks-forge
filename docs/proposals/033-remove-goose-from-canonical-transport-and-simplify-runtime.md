# Proposal 033: Complete Goose Removal and ACP-Only Runtime Architecture

| Field | Value |
|---|---|
| Date | 2026-04-09 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Remove all Goose code and refactor transport, MCP, session, configuration, and provider platform layers to ACP-only architecture. |
| Goal | Zero Goose references in the codebase. Every runtime path is ACP. |

---

## 1. Context and Motivation

Goose was the original runtime transport. The ACP migration (P026 → P030) introduced a parallel ACP seam that now covers five provider adapters. With P030 proven, Goose is dead weight:

- Transport layer carries dual paths (Goose REST/SSE + ACP JSON-RPC subprocess)
- MCP resolution has Goose-specific registry, namespace, and validation logic
- Session bridge has Goose extension reconciliation branches
- Configuration carries `gooseServer*` fields and `CHAINWORKS_GOOSE_*` env vars
- Provider platform has Goose-backed families alongside ACP families
- Every `Goose`-prefixed filename confuses the architecture

This proposal removes Goose completely and refactors each layer to be ACP-native.

---

## 2. Hard Prerequisite Gate

Implementation cannot begin until P030 passes audit to `Implemented / Ready`. The `proposal-033` gate asserts P030 green before running any P033 tests.

---

## 3. Layer-by-Layer Refactoring

### 3.1 Transport Layer

**Current**: `DefaultRuntimeTransportFactory` has a `gooseTransport` field, a `"goose"` switch case, and Goose-specific `fatalError` paths.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `gooseTransport` field | Factory only holds `transportsByFamily` cache |
| Delete `"goose"` case in switch | Unknown families already throw `unknownAdapterFamily` |
| Delete `GooseTransportAPI` enum | No `bespoke`/`goose_server` distinction needed |
| Delete `resolveGooseTransport()` in ExecutionService | ACP transports are self-contained |
| Delete `liveRuntimeConfiguration` Goose fields | Replace with `runtimeEndpointOverrides` if needed |
| Delete `GooseServerTransport.swift` | — |
| Delete `GooseServerManager.swift` | — |
| Delete `GooseStreamEventMapper.swift` | — |
| Delete `GooseTransport.swift` | — |
| Delete `Engine/GooseAdapter/` directory | — |

**Result**: Factory only instantiates ACP transports. No Goose code in transport layer.

### 3.2 MCP Layer

**Current**: `MCPPolicyResolver` has `runtimeNamespace == "goose"` checks, `GooseExtensionRegistryReader` reads `~/.config/goose/config.yaml`, and validation blocks on missing Goose registry.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `GooseExtensionRegistryReader` | Each ACP adapter has its own `RuntimeExtensionRegistryProvider` conformer |
| Delete `runtimeNamespace == "goose"` branches in resolver | Only ACP namespaces remain (`claude_agent`, `gemini_cli`, `codex`, `auggie`, `junie`) |
| Delete Goose-specific error messages | Replace "Goose extension registry" → "Runtime extension registry" (partially done in P029) |
| Delete `goose` case in `effectiveRuntimeNamespace` | — |
| Delete `usesGooseExecutionPath` property | — |
| Delete `CHAINWORKS_GOOSE_CONFIG_PATH` env var handling | — |
| Delete `examples/goose/goose-config-fixture.yaml` | Replace with ACP-native test fixtures |
| Change resolver default | Currently defaults to `"goose"` when no `runtimeProfileID`. After P033: require explicit `runtimeProfileID` or throw |

**Result**: MCP resolver works only with ACP runtime namespaces and adapter-specific registry providers.

### 3.3 Session Bridge Layer

**Current**: `GooseSessionBridge.swift` (already misnamed — it serves ACP too) has Goose extension reconciliation branches at lines 65-98.

**Refactor**:

| Change | Detail |
|--------|--------|
| Rename `GooseSessionBridge.swift` → `RuntimeSessionBridge.swift` | Already used the type name `RuntimeSessionBridge` internally |
| Delete Goose extension reconciliation branches | ACP transports receive `mcpServers` directly, no Goose extension ID translation needed |
| Delete `GooseSystemPromptStore` dependency | ACP transports embed system prompt in `session/new` or `session/prompt` directly |
| Simplify `resolveMCPPolicy` | No Goose-specific blocking/warning conditions |

**Result**: Session bridge is transport-neutral, delegates all transport-specific behavior to the `RuntimeTransportProtocol` implementation.

### 3.4 Executor Layer

**Current**: `GooseAgentExecutor.swift` is the real `RuntimeAgentExecutor`. Has `gooseTransportForCancellation` property and Goose-specific MCP dispatch.

**Refactor**:

| Change | Detail |
|--------|--------|
| Rename `GooseAgentExecutor.swift` → `RuntimeAgentExecutor.swift` | Matches the actual class name |
| Delete `gooseTransportForCancellation` | ACP transports handle their own subprocess termination via `closeSession` |
| Delete Goose-specific MCP dispatch in `resolveMCPPolicy` | Already dispatches by adapter family; remove `"goose"` path |
| Update `resolveSession` to not pass `requestedExtensions` | ACP uses `mcpServers`, not Goose extension IDs |

### 3.5 Configuration Layer

**Current**: `AppConfiguration` has six `gooseServer*` fields. `BootstrapConfigurationResolver` reads `CHAINWORKS_GOOSE_*` env vars.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `gooseServerHost`, `gooseServerPort`, `gooseServerTLS`, `gooseServerAutostart`, `gooseServerBinaryPath`, `gooseServerSecretKey` | — |
| Delete `gooseServerBaseURL` computed property | — |
| Delete `defaultGooseServerBinaryPath()` | — |
| Delete `CHAINWORKS_GOOSE_BASE_URL` handling | — |
| Delete `CHAINWORKS_GOOSE_API_KEY` handling | — |
| Delete `CHAINWORKS_GOOSE_BINARY_PATH` handling | — |
| Delete `CHAINWORKS_GOOSE_FIXTURE_MODE` handling | Replace with `CHAINWORKS_ACP_FIXTURE_MODE` |
| Delete `GooseServerManager` and all autostart logic | ACP transports are subprocess-managed, not server-managed |

**Result**: Configuration has zero Goose fields. ACP adapters manage their own lifecycle.

### 3.6 Provider Platform Layer

**Current**: `ProviderFamily` has Goose-backed `.codex`, `.claude`, `.gemini` alongside ACP `.codexACP`, `.auggie`, `.junie`. `ProviderTransport` has `.gooseServer`.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `.codex` (Goose-backed) → keep `.codexACP` | Codex ACP is the only Codex runtime |
| Rename `.claude` → `.claudeACP` | Claude Agent ACP is the only Claude runtime |
| Rename `.gemini` → `.geminiACP` | Gemini CLI ACP is the only Gemini runtime |
| Delete `ProviderTransport.gooseServer` | Only `.cli` and `.httpAPI` remain (or collapse to just `.acpStdio`) |
| Delete `GooseProviderConnectionAssistant.swift` | — |
| Delete `GooseProviderConnectionAssistantView.swift` | — |
| Delete `verifyGooseServerProvider()` in ProviderAdapterSupport | — |
| Delete `GooseServerReachability` enum | — |
| Delete Goose-backed `ProviderAdapter` implementations | Keep ACP adapters only |
| Delete Goose seeded settings | Only ACP providers are seeded |
| Delete `gooseFirstPreferred` property | — |
| Change resolver default from `"goose"` to throw | Require explicit `runtimeProfileID` |

**Result**: Provider platform has five ACP families, no Goose.

### 3.7 Fixture / Test Layer

**Current**: `FixtureGooseTransport` provides test scenarios. Many tests use it.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `FixtureGooseTransport.swift` | Replace with `FixtureACPTransport` |
| Create `FixtureACPTransport` | Same scenario enum, same deterministic behavior, `mcpRuntimeNamespace = "claude_agent"` (or parametric) |
| Rename `GooseSessionBridgeTests.swift` → `RuntimeSessionBridgeTests.swift` | Remove Goose fixtures |
| Rename `GooseAgentExecutorTests.swift` → `RuntimeAgentExecutorTests.swift` | Use ACP fixtures |
| Delete `GooseServerTransportTests.swift` | — |
| Delete `GooseStreamEventMapperTests.swift` | — |
| Delete `GooseServerLiveIntegrationTests.swift` | — |
| Update `MVPGoldenRunTests`, `FullMVPDeliveryTests`, `Proposal022Tests` | Use `FixtureACPTransport` |
| Delete `SharedMocks.swift` Goose stubs | Replace with ACP stubs |
| Delete `examples/goose/goose-config-fixture.yaml` | — |
| Delete `test-gate.sh` `PORTABLE_GOOSE_CONFIG_PATH` | — |

### 3.8 UI Layer

| Surface | Change |
|---------|--------|
| `ProviderSettingsView.swift` | Delete Goose transport setup; ACP-only |
| `FirstRunSetupWizard.swift` | Delete Goose server config steps; ACP provider selection only |
| `IdeaListView.swift` | Replace "Goose server" readiness → "ACP runtime" readiness |
| `PilotReadinessView.swift` | Delete Goose readiness checks; adapter-neutral checks only |
| `RunsHomeView.swift` | Normalize trust display; no "server_verified" Goose vocabulary |
| `GooseProviderConnectionAssistantView.swift` | **Delete** |

### 3.9 Docs Layer

| Doc | Change |
|-----|--------|
| `reference/goose-server-transport.md` | **Delete** |
| `reference/operator-experience.md` | Remove all Goose mentions |
| `reference/provider-platform.md` | Rewrite for ACP-only |
| `reference/test-gates.md` | Remove Goose gates |

---

## 4. Handling Existing Goose-Bound Runs

Runs with `adapterFamily == "goose"` or `transport == "goose_server"` in their frozen bindings:

1. `ResumeManager.classifyRun()` checks frozen bindings.
2. If any binding uses Goose → `.cannotResume(run, reason: "Goose runtime is no longer supported.")`.
3. Run status → `.blocked`, `driftDetails` → removal message.
4. Operator sees: "This run used the Goose runtime which has been removed. Archive it or create a new run with an ACP provider."

No migration. No conversion. Old Goose runs are blocked forever.

---

## 5. Skills Layer

**No changes needed.** The skill system (`SkillResolver`, `SkillInjector`, `ResolvedSkill`) is already transport-neutral. Skills are injected via system prompt prepending, which works identically for all ACP transports. P033 does not touch the skills layer.

---

## 6. Trust Model

| Persisted Value | Display |
|----------------|---------|
| `fixture_verified` | "Fixture" |
| `server_unverified` | "Legacy (unverified)" — historical Goose runs |
| `server_verified` | "Legacy (verified)" — historical Goose runs |
| `runtime_verified` | "Verified" |
| `runtime_unverified` | "Unverified" |
| `nil` | "Unknown" |

Reader normalizes legacy values on read. No data migration.

---

## 7. Acceptance Criteria

1. Zero files in `Engine/GooseAdapter/`.
2. Zero `Goose` in any Swift source filename.
3. `ProviderFamily` has no Goose-backed cases.
4. `ProviderTransport.gooseServer` does not exist.
5. `AppConfiguration` has zero `gooseServer*` fields.
6. Zero `CHAINWORKS_GOOSE_*` env vars in codebase.
7. `MCPPolicyResolver` has zero `"goose"` namespace references.
8. `DefaultRuntimeTransportFactory` has no `gooseTransport` field.
9. `ResumeManager` blocks Goose-bound runs with explicit error.
10. All tests compile and pass using `FixtureACPTransport`.
11. UI surfaces have zero "Goose" in operator-facing strings.
12. `proposal-033` gate passes with P030 prerequisite.

---

## 8. Risks

- **Scope**: ~50 files touched. Mitigated by exhaustive inventory and per-layer phasing.
- **Old runs**: Goose runs become permanently blocked. Mitigated by clear error message.
- **Binary dependencies**: Operators who only had `goosed` installed need ACP binaries. Mitigated by P030 prerequisite.

---

## 9. Alternatives Considered

### 9.1 Keep Goose as compatibility adapter

Rejected. Doubles maintenance, confuses naming, delays simplification.

### 9.2 Migrate Goose runs to ACP

Rejected. Frozen bindings are provenance truth. Converting them is corruption.

### 9.3 Gradual deprecation

Rejected. Single-developer app, no release cadence. Clean cut.
