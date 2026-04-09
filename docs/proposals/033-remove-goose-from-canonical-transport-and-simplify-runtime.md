# Proposal 033: Complete Goose Removal and ACP-Only Runtime

| Field | Value |
|---|---|
| Date | 2026-04-09 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Completely remove Goose from the codebase — all transport code, adapter code, provider families, server management, extension registry readers, UI references, docs, and test fixtures. Existing Goose-bound runs are blocked with an explicit unsupported-runtime error. |
| Goal | The application has zero Goose code paths. Runtime is ACP-only. |

---

## 1. Context and Motivation

Goose was the original runtime transport. P026 introduced the ACP transport seam. P030 expanded it to five ACP adapters. After P030 is proven, Goose serves no purpose:

- No new workflows should use Goose.
- Maintaining Goose compatibility doubles transport surface area.
- Goose-specific code confuses naming throughout the codebase (`GooseAgentExecutor`, `GooseSessionBridge`, `GooseServerManager`, etc.).
- MCP resolution carries Goose-specific registry logic that does not apply to ACP.

This proposal removes Goose entirely. No compatibility mode, no fallback, no "optional adapter." Gone.

---

## 2. Hard Prerequisite Gate

This proposal cannot begin implementation until P030 passes its full audit to `Implemented / Ready`. The `proposal-033` gate asserts P030 green before running any P033 tests.

---

## 3. What Gets Deleted

### 3.1 Transport / Adapter Code

| File | Action |
|------|--------|
| `Engine/GooseAdapter/GooseServerTransport.swift` | **Delete** |
| `Engine/GooseAdapter/GooseServerManager.swift` | **Delete** |
| `Engine/GooseAdapter/GooseStreamEventMapper.swift` | **Delete** |
| `Engine/GooseAdapter/GooseTransport.swift` | **Delete** |
| `Engine/GooseAdapter/` directory | **Delete** |
| `Engine/FixtureGooseTransport.swift` | **Delete** — replace with `FixtureACPTransport` |

### 3.2 Provider Platform

| Item | Action |
|------|--------|
| `ProviderFamily.codex` (Goose-backed) | **Delete** case |
| `ProviderFamily.claude` (Goose-backed) | **Delete** case |
| `ProviderFamily.gemini` (Goose-backed) | **Delete** case |
| `ProviderTransport.gooseServer` | **Delete** case |
| `GooseProviderConnectionAssistant.swift` | **Delete** |
| `GooseProviderConnectionAssistantView.swift` | **Delete** |
| Seeded Goose-backed `ConfiguredProvider` entries | **Delete** |
| `CodexProviderAdapter` / `ClaudeProviderAdapter` / `GeminiProviderAdapter` (Goose-backed) | **Delete** |

Remaining provider families: `.codexACP`, `.claude` (→ renamed to `.claudeACP`), `.gemini` (→ renamed to `.geminiACP`), `.auggie`, `.junie`.

### 3.3 MCP / Registry

| Item | Action |
|------|--------|
| `GooseExtensionRegistryReader` | **Delete** |
| `GooseExtensionDefinition` → already `RuntimeExtensionDefinition` | Keep |
| `GooseExtensionRegistrySnapshot` → already `RuntimeExtensionRegistrySnapshot` | Keep |
| `mcp_server_registry` Goose-specific entries in `agents.yaml` | **Delete** Goose entries |
| Goose namespace in `effectiveRuntimeNamespace` | **Delete** `"goose"` case |

### 3.4 Executor / Session Bridge

| Item | Action |
|------|--------|
| `GooseAgentExecutor.swift` | **Rename** → `RuntimeAgentExecutor.swift` |
| `GooseSessionBridge.swift` | **Rename** → `RuntimeSessionBridge.swift` |
| Goose-specific cancellation path in executor | **Delete** |
| `gooseTransportForCancellation` property | **Delete** |

### 3.5 Transport Factory

| Item | Action |
|------|--------|
| `DefaultRuntimeTransportFactory.gooseTransport` field | **Delete** |
| `"goose"` case in factory switch | **Delete** — unknown family already throws |
| `SingleTransportFactory` | Keep (test utility, transport-neutral) |

### 3.6 Tests

| File | Action |
|------|--------|
| `GooseServerTransportTests.swift` | **Delete** |
| `GooseStreamEventMapperTests.swift` | **Delete** |
| `GooseServerLiveIntegrationTests.swift` | **Delete** |
| `GooseSessionBridgeTests.swift` | **Rename** → `RuntimeSessionBridgeTests.swift`, remove Goose fixtures |
| `GooseAgentExecutorTests.swift` | **Rename** → `RuntimeAgentExecutorTests.swift`, use ACP fixtures |
| `SharedMocks.swift` Goose stubs | **Delete** or replace with ACP stubs |

### 3.7 UI / Views

| Surface | Action |
|---------|--------|
| `ProviderSettingsView.swift` Goose transport setup | **Delete** Goose path |
| `FirstRunSetupWizard.swift` Goose server config | **Delete** Goose steps |
| `IdeaListView.swift` "Goose server" readiness | **Replace** with "ACP runtime" readiness |
| `PilotReadinessView.swift` Goose readiness checks | **Replace** with adapter-neutral checks |
| `RunsHomeView.swift` Goose trust rendering | **Replace** with ACP trust vocabulary |
| `GooseProviderConnectionAssistantView.swift` | **Delete** |

### 3.8 Docs / Reference

| Doc | Action |
|-----|--------|
| `reference/goose-server-transport.md` | **Delete** |
| All Goose mentions in `reference/operator-experience.md` | **Delete** |
| All Goose mentions in `reference/provider-platform.md` | **Rewrite** for ACP-only |

### 3.9 Configuration / Settings

| Item | Action |
|------|--------|
| `AppConfiguration.gooseServer*` fields | **Delete** |
| `GooseServerManager` autostart logic | **Delete** |
| `CHAINWORKS_GOOSE_*` environment variables | **Delete** |
| `goose-config-fixture.yaml` | **Delete** |

---

## 4. Handling Existing Goose-Bound Runs

Runs persisted with `adapterFamily == "goose"` or `transport == "goose_server"` cannot execute. On resume:

1. `ResumeManager.classifyRun()` checks `adapterFamily` in the frozen provider binding snapshot.
2. If any binding uses `"goose"` adapter family, return `.cannotResume(run, reason: "Goose runtime is no longer supported. This run was created with the Goose transport which has been removed.")`.
3. Run status → `.blocked`, `driftDetails` → the removal message.
4. Operator sees: "This run used the Goose runtime which is no longer available. Archive it or create a new run with an ACP provider."

No migration, no conversion. Old Goose runs are dead. New runs use ACP.

---

## 5. MCP Simplification

With Goose gone:

- `mcp_server_registry` no longer needs Goose namespace entries. Keep only ACP namespace mappings.
- `MCPPolicyResolver` no longer needs `runtimeNamespace == "goose"` special cases.
- `GooseExtensionRegistryReader` is deleted. Each ACP adapter has its own `RuntimeExtensionRegistryProvider` conformer (already exists for Codex via `CodexExtensionRegistryReader`).
- Preflight MCP validation uses adapter-family-specific registry providers exclusively.

`mcp_profile` and `mcp_server_registry` stay in the catalog schema for now — they serve ACP namespaces too. Only the Goose-specific entries and code paths are removed.

---

## 6. Trust Model

Historical trust values on persisted runs:

| Persisted Value | Display |
|----------------|---------|
| `fixture_verified` | "Fixture" |
| `server_unverified` | "Legacy (unverified)" |
| `server_verified` | "Legacy (verified)" |
| `runtime_verified` | "Verified" |
| `runtime_unverified` | "Unverified" |
| `nil` | "Unknown" |

New runs only write `fixture_verified`, `runtime_verified`, or `runtime_unverified`. The reader normalizes legacy values for display only.

---

## 7. Acceptance Criteria

1. Zero Goose source files remain in `Engine/GooseAdapter/`.
2. `ProviderFamily` has no Goose-backed cases (`.codex`, `.claude`, `.gemini` → deleted or renamed to ACP variants).
3. `ProviderTransport.gooseServer` does not exist.
4. `DefaultRuntimeTransportFactory` has no `gooseTransport` field and no `"goose"` case.
5. `ResumeManager` blocks Goose-bound runs with an explicit unsupported-runtime error.
6. MCP resolution has no Goose-specific code paths.
7. UI surfaces have zero "Goose" references in operator-facing strings.
8. `AppConfiguration` has no `gooseServer*` fields.
9. All tests compile and pass without any Goose transport fixtures.
10. `proposal-033` gate in `test-gate.sh` passes on the canonical tree.
11. `proposal-033` gate includes P030 prerequisite check.

---

## 8. Risks

- **Data loss perception**: Operators with existing Goose runs see them blocked. Mitigated by clear error message and archive option.
- **Binary dependency**: Some environments may still have `goosed` but no ACP binaries. Mitigated by P030 prerequisite — ACP providers must be proven before Goose removal begins.
- **Scope creep**: Goose removal touches many files. Mitigated by exhaustive inventory in Section 3.

---

## 9. Alternatives Considered

### 9.1 Keep Goose as compatibility adapter

Rejected. Doubles maintenance surface, confuses naming, and creates false safety net. Clean removal is simpler.

### 9.2 Migrate existing Goose runs to ACP

Rejected. Provider bindings are frozen at run start. Converting them would corrupt provenance truth. Blocking with a clear message is honest.

### 9.3 Gradual deprecation over multiple releases

Rejected. There is no release cadence — this is a single-developer app. Clean cut is appropriate.
