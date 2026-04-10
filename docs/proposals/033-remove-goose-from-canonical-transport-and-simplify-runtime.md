# Proposal 033: Complete Goose Removal and ACP-Only Runtime Architecture

| Field | Value |
|---|---|
| Date | 2026-04-09 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Remove all Goose runtime code and refactor transport, MCP, session, configuration, and provider platform layers to ACP-only architecture. |
| Goal | Zero Goose runtime references in Swift source, configuration, and operator-facing surfaces. Brand/design assets that use geese as visual metaphor are out of scope. Every runtime path is ACP. |

---

## 1. Context and Motivation

Goose was the original runtime transport. The ACP migration (P026 → P030) introduced a parallel ACP seam that now covers five provider adapters. With the second-wave ACP expansion (P030) proven, Goose is dead weight:

- Transport layer carries dual paths (Goose REST/SSE + ACP JSON-RPC subprocess)
- MCP resolution has Goose-specific registry, namespace, and validation logic
- Session bridge has Goose extension reconciliation branches
- Configuration carries `gooseServer*` fields and `CHAINWORKS_GOOSE_*` env vars
- Provider platform has Goose-backed families alongside ACP families
- Every `Goose`-prefixed filename confuses the architecture

This proposal removes Goose completely and refactors each layer to be ACP-native.

---

## 2. Hard Prerequisite Gate and Proof Lane

Implementation cannot begin until the second-wave ACP proposal (P030) passes audit to `Implemented / Ready`.

**Note on gate naming**: The second-wave ACP proposal file is `030-acp-second-wave-runtime-profiles-codex-auggie-junie.md`, but the canonical gate in `test-gate.sh` is registered as `proposal-029` with array `PROPOSAL_029_TESTS`. This is the historical gate name from before the proposal was renumbered. P033 uses `proposal-029` as the prerequisite gate name because that is what `test-gate.sh` actually implements. If the gate is renamed to `proposal-030` before P033 implementation, update the prerequisite accordingly.

**Gate definition for `test-gate.sh`**:

```bash
PROPOSAL_033_TESTS=(
  "Chainworks ForgeTests/Proposal033Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"    # renamed from GooseSessionBridgeTests
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"     # renamed from GooseAgentExecutorTests
  "Chainworks ForgeTests/MVPGoldenRunTests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)
```

```bash
proposal-033|p033)
  check_idle_environment allow_app
  guard_direct_run_insertion
  # Prerequisite: second-wave ACP gate must be green
  log "Prerequisite: proposal-029 gate (second-wave ACP)"
  run_targeted_tests "proposal-029-prereq" "${PROPOSAL_029_TESTS[@]}"
  # P033-specific proof
  run_build "proposal-033"
  run_targeted_tests "proposal-033" "${PROPOSAL_033_TESTS[@]}"
  ;;
```

**`Proposal033Tests` must prove**:

1. `ProviderSettingsStore.migrateFromGooseEra()` correctly migrates persisted Goose-era local JSON (family, transport, endpoint, displayName, capabilities, adapterVersion, preferredProviderIDsByFamily keys).
2. `SettingsTransferService` import path applies the same migration to imported Goose-era JSON before merging — Claude/Gemini rows preserve UUID and credentials, Codex rows are deleted (re-auth required on target machine).
3. `DefaultRuntimeTransportFactory` has no `gooseTransport` field and throws for `"goose"` family.
4. `MCPPolicyResolver` has no `"goose"` namespace branches.
5. `ResumeManager` blocks Goose-bound runs with explicit error.
6. `FixtureACPTransport` produces valid execution proof (replaces `FixtureGooseTransport` scenarios).
7. `effectiveRuntimeNamespace` has no `"goose"` case.
8. Zero `"Goose"` in operator-facing preflight messages.

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
| Delete `ExecutionService.gooseServerManager` field | ACP transports don't use a central server manager |
| Delete `Chainworks_ForgeApp.swift` Goose bootstrap | Remove `gooseServerManager` creation, bootstrap, and passing to ExecutionService (~15 references) |
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

### 3.4a Persistent Model Migration (`AgentExecution.gooseSessionID`)

**Current**: `AgentExecution` (SwiftData `@Model`) has a persisted stored property `gooseSessionID: String?` (line 16). A computed alias `runtimeSessionID` wraps it (lines 19-22). `SupportBundleExporter` exports the `gooseSessionID` key. `domain-model.md` documents it.

**Decision**: **Rename the stored property** from `gooseSessionID` to `runtimeSessionID`, making the current alias the real property.

**SwiftData migration**: SwiftData lightweight migration handles property renames when the old and new types match. Add `@Attribute(originalName: "gooseSessionID")` to preserve data from existing stores:

```swift
@Attribute(originalName: "gooseSessionID")
var runtimeSessionID: String?
```

**Downstream changes**:

| Owner | Change |
|-------|--------|
| `AgentExecution.swift` | Rename stored property, delete computed alias |
| `SupportBundleExporter.swift` | Export as `"runtimeSessionID"` — no backward-compat key |
| `domain-model.md` | Update field name |
| Any direct `gooseSessionID` references | Replace with `runtimeSessionID` |

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
| Delete `ProviderTransport.gooseServer` | Remaining cases: `.cli`, `.httpAPI`, `.localBridge` |
| Delete `GooseProviderConnectionAssistant.swift` | — |
| Delete `GooseProviderConnectionAssistantView.swift` | — |
| Delete `verifyGooseServerProvider()` in ProviderAdapterSupport | — |
| Delete `GooseServerReachability` enum | — |
| Delete Goose-backed `ProviderAdapter` implementations | Keep ACP adapters only |
| Delete Goose seeded settings | Only ACP providers are seeded |
| Delete `gooseFirstPreferred` property | — |
| Change resolver default from `"goose"` to throw | Require explicit `runtimeProfileID` |

**Result**: Provider platform has five ACP families, no Goose.

### 3.6a Durable Settings Migration

`ProviderFamily` and `ProviderTransport` are `Codable` enums with `rawValue` serialization. Operator machines have persisted `provider-settings.json` and `chainworks-settings.json` containing Goose-era raw values. Deleting enum cases without migration breaks deserialization.

**Migration table for `ProviderFamily.rawValue`**:

| Persisted rawValue | Action | New rawValue |
|--------------------|--------|-------------|
| `"codex"` | Delete provider entry | — (Goose-backed Codex removed) |
| `"claude"` | Migrate → `.claudeACP` | `"claudeACP"` |
| `"gemini"` | Migrate → `.geminiACP` | `"geminiACP"` |
| `"codexACP"` | Keep | `"codexACP"` |
| `"auggie"` | Keep | `"auggie"` |
| `"junie"` | Keep | `"junie"` |

**Migration table for `ProviderTransport.rawValue`**:

| Persisted rawValue | Action | New rawValue |
|--------------------|--------|-------------|
| `"goose_server"` | Migrate → `.cli` | `"cli"` |
| `"cli"` | Keep | `"cli"` |
| `"httpAPI"` | Keep | `"httpAPI"` |
| `"localBridge"` | Keep | `"localBridge"` |

**Migration table for `preferredProviderIDsByFamily` keys**:

| Persisted key | Action | New key |
|---------------|--------|---------|
| `"codex"` | Delete entry | — |
| `"claude"` | Rename key → `"claudeACP"` | `"claudeACP"` |
| `"gemini"` | Rename key → `"geminiACP"` | `"geminiACP"` |
| Other keys | Keep | Same |

**Implementation**: `ProviderSettingsStore` gains a **raw JSON pre-decode migration** that runs before `JSONDecoder` ever touches the payload. This is critical: once Goose-era enum cases (`.codex`, `.claude`, `.gemini`, `.gooseServer`) are deleted from Swift source, `JSONDecoder` will fail on any JSON containing those raw values. The migration must happen on raw `Data`/`[String: Any]`, not on typed models.

**Two payload shapes require two schema-specific raw migrators:**

1. **`provider-settings.json`** (local persistence) — top-level shape is `ProviderSettings`: `configuredProviders` array and `preferredProviderIDsByFamily` dict at root level.

2. **`chainworks-settings.json`** (transfer/import) — top-level shape is `ExportableSettingsPackage`: `providerSettings` (nested `ProviderSettings`), `secretPlaceholders` array, and `appConfiguration` at root level.

Each shape gets its own raw migrator:

```swift
// Shape 1: local ProviderSettings (used in ProviderSettingsStore.init)
static func migrateRawProviderSettings(_ data: Data) throws -> Data {
    guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return data }
    guard (json["migration_version"] as? Int ?? 0) < 1 else { return data }
    migrateProviderSettingsFields(&json)
    json["migration_version"] = 1
    return try JSONSerialization.data(withJSONObject: json)
}

// Shape 2: wrapped ExportableSettingsPackage (used in SettingsTransferService.importSettings)
static func migrateRawTransferPackage(_ data: Data) throws -> Data {
    guard var json = try JSONSerialization.jsonObject(with: data) as? [String: Any] else { return data }
    guard var nested = json["providerSettings"] as? [String: Any] else { return data }
    guard (nested["migration_version"] as? Int ?? 0) < 1 else { return data }
    migrateProviderSettingsFields(&nested)
    nested["migration_version"] = 1
    json["providerSettings"] = nested
    // Rewrite secretPlaceholders: drop entries keyed to deleted Codex UUIDs
    if var placeholders = json["secretPlaceholders"] as? [String] {
        let deletedUUIDs = findDeletedCodexUUIDs(nested)
        placeholders.removeAll { key in deletedUUIDs.contains { key.contains($0) } }
        json["secretPlaceholders"] = placeholders
    }
    return try JSONSerialization.data(withJSONObject: json)
}

// Shared: the actual field rewrites (family, transport, endpoint, etc.)
private static func migrateProviderSettingsFields(_ json: inout [String: Any]) { ... }
```

Both return migrated `Data` that `JSONDecoder` can then decode with the new enum cases.

**Migration steps** (all on raw `[String: Any]`, never on typed models):

1. Read raw JSON from disk as `[String: Any]`.
2. Check `migration_version` — skip if already ≥ 1.
3. **Delete Goose-era Codex rows entirely**: Remove all entries in `configuredProviders` array where `family == "codex"`. The Goose-era `.codex` family has no ACP continuation — `.codexACP` is a distinct family. The entry is discarded. Operator gets a fresh seeded `.codexACP` entry after decode.
3. **Migrate surviving Claude/Gemini rows** with full field semantics:
   - `family: "claude"` → `"claudeACP"`, `family: "gemini"` → `"geminiACP"`
   - `transport: "goose_server"` → `"cli"` (ACP adapters are subprocess-based)
   - `endpoint`: **clear to nil** — Goose endpoint (`https://127.0.0.1:51200`) is meaningless for ACP subprocess
   - `authMode`: **keep as-is** — API key auth remains valid for ACP
   - `displayName`: rewrite `"Claude Goose"` → `"Claude ACP"`, `"Gemini Goose"` → `"Gemini ACP"`. Other custom names keep as-is.
   - `capabilities`: **replace with `ProviderCapabilities.default(for: .claudeACP)`** / `.geminiACP` — Goose-era capability flags may not match ACP reality
   - `adapterVersion`: rewrite `"v1"` → `"acp-v1"`
   - `isEnabled`: set to `true` (these were active Goose providers, keep them active as ACP)
4. Rewrite `preferredProviderIDsByFamily` keys per migration table above (raw string key rewrite).
5. Set `migration_version: 1` in the raw JSON.
6. Serialize back to `Data` and persist.
7. Return migrated `Data` for `JSONDecoder` to decode with new enum cases.

**Call sites** — each invokes its schema-specific migrator directly, no umbrella dispatcher:

- `ProviderSettingsStore.init(fileURL:)` calls `migrateRawProviderSettings(_:)` on raw file data before `JSONDecoder.decode(ProviderSettings.self, ...)`.
- `SettingsTransferService.importSettings(_:)` calls `migrateRawTransferPackage(_:)` on raw import data before `JSONDecoder.decode(ExportableSettingsPackage.self, ...)`.

**Credential / UUID migration**: Secrets are keyed by `"provider.\(UUID)"` in `KeychainSecretStore` (via `ProviderAdapterSupport.secretKey(for:)`). Migration semantics:

- **Claude/Gemini rows (migrated)**: The `ConfiguredProvider` row is mutated in-place — `id` (UUID) is **preserved**. The Keychain secret remains valid because it's keyed by UUID, not family name. No re-auth needed.
- **Codex rows (deleted)**: The row is removed. Its Keychain secret becomes orphaned — harmless, never accessed. The fresh seeded `.codexACP` entry gets a new UUID. Operator must configure credentials for the new entry. This is an explicit **re-auth requirement**, not a silent loss.
- **SettingsTransferService**: Transfer placeholders are also keyed by UUID. Migrated Claude/Gemini UUIDs survive. Deleted Codex UUIDs are dropped from the transfer bundle.

**Decision**: Preserve UUID for migrated rows (Claude/Gemini). Accept re-auth for deleted rows (Codex). No attempt to transfer secrets across UUID boundaries.

**SettingsTransferService**: Import path runs the same migration on imported JSON before merging. Export path writes current (already-migrated) state — no special handling.

**Canonical outcome for each Goose-era row**:

| Goose-era Row | Outcome |
|---------------|---------|
| Codex Goose (`family: "codex"`, `transport: "goose_server"`) | **Deleted** — replaced by seeded `.codexACP` |
| Claude Goose (`family: "claude"`, `transport: "goose_server"`) | **Migrated** → `.claudeACP`, `transport: "cli"`, endpoint cleared, capabilities reset |
| Gemini Goose (`family: "gemini"`, `transport: "goose_server"`) | **Migrated** → `.geminiACP`, `transport: "cli"`, endpoint cleared, capabilities reset |
| Any non-Goose provider (already ACP) | **Unchanged** |

**YAML/provider identifiers in catalog**: `agents.yaml` `provider` fields use string identifiers (`codex`, `claude_code`, `gemini`). These are `runtimeProviderIdentifier` mapped from `ProviderFamily`, not the rawValue itself. Migration:

| YAML `provider` | Action | New value |
|-----------------|--------|-----------|
| `"codex"` | Migrate → `"codex_acp"` | `"codex_acp"` |
| `"claude_code"` | Migrate → `"claude_acp"` | `"claude_acp"` |
| `"gemini"` | Migrate → `"gemini_acp"` | `"gemini_acp"` |

**Baseline vocabulary** (`current-system-baseline.md`): Update `codex / claude_code / gemini` → `codex_acp / claude_acp / gemini_acp / auggie / junie`.

### 3.7 Fixture / Test Layer

**Current**: `FixtureGooseTransport` provides test scenarios. Many tests use it.

**Refactor**:

| Change | Detail |
|--------|--------|
| Delete `FixtureGooseTransport.swift` | Replace with `FixtureACPTransport` |
| Create `FixtureACPTransport` | Same scenario enum, same deterministic behavior, parametric `mcpRuntimeNamespace` defaulting to `"claude_agent"` |
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
| `reference/operator-experience.md` | Remove all Goose runtime mentions |
| `reference/provider-platform.md` | Rewrite for ACP-only; update provider vocabulary |
| `reference/test-gates.md` | Remove Goose gates |
| `reference/acp-runtime-transport.md` | Remove Goose comparison/migration narrative; ACP is the only transport |
| `reference/current-system-baseline.md` | Update provider vocabulary from `codex/claude_code/gemini` to ACP families |
| `reference/README.md` | Remove Goose transport references |
| `reference/chainworks_forge_design_kit_v1.md` | **No change** — brand/design uses geese as visual metaphor, not runtime reference |
| `reference/goose-provider-remediation.md` | **Delete** — Goose remediation no longer applicable |
| `reference/live-provider-execution-slice.md` | Rewrite to describe ACP-only execution; remove Goose transport path |
| `reference/workflow-execution-engine.md` | Remove Goose transport assumptions; describe ACP-only executor |
| `reference/per-agent-mcp-policy-and-runtime-validation.md` | Remove Goose extension registry references; ACP registries only |
| `reference/test-suite-architecture.md` | Remove Goose fixture/test references; describe ACP fixture strategy |
| `reference/provider-binding-truth.md` | Rewrite: remove Goose transport binding examples; ACP bindings only |
| `reference/run-control.md` | Rewrite: remove Goose runtime control references; ACP session control only |
| `reference/skill-resolution-and-runtime-integration.md` | Rewrite: remove Goose-era skill injection path references; skills are already transport-neutral but doc language may still reference Goose session |
| `reference/agent-ui-test-execution.md` | Rewrite: remove Goose-backed UI test assumptions; ACP fixture strategy |
| `reference/runtime-contract.md` | Update `required now: codex, claude_code, gemini` → `codex_acp, claude_acp, gemini_acp, auggie, junie` |
| `reference/mvp-sign-off.md` | Update provider list from `codex / claude_code / gemini` to ACP families |
| `Support/MVPBoundaryPolicy.swift` | Update `canonicalMVPProviders` set and `providerDisplayNames` to ACP identifiers |
| `.review-baselines/current-system-baseline.md` | Update baseline provider vocabulary to ACP-only |

**Scope clarification**: "Zero Goose runtime references" means zero in Swift source, configuration files, environment variable names, and operator-facing UI strings. Brand/design assets (`chainworks_forge_design_kit_v1.md`) that use geese as visual metaphor are explicitly out of scope.

---

## 4. Handling Existing Goose-Bound Runs

Runs with `adapterFamily == "goose"` or `transport == "goose_server"` in their frozen bindings:

1. `ResumeManager.classifyRun()` checks frozen bindings.
2. If any binding uses a removed adapter family → `.cannotResume(run, reason: "This run requires a runtime adapter that is no longer available. Archive it or create a new run.")`.
3. Run status → `.blocked`, `driftDetails` → removal message.
4. Operator sees: "This run requires a runtime adapter that is no longer available. Archive it or create a new run with a supported provider."

No migration. No conversion. Runs with removed adapter families are blocked forever. The operator-facing message does not name the removed runtime — it is transport-neutral.

---

## 5. Skills Layer

**No changes needed.** The skill system (`SkillResolver`, `SkillInjector`, `ResolvedSkill`) is already transport-neutral. Skills are injected via system prompt prepending, which works identically for all ACP transports. P033 does not touch the skills layer.

---

## 6. Trust Model

| Persisted Value | Display |
|----------------|---------|
| `fixture_verified` | "Fixture" |
| `server_unverified` | "Legacy (unverified)" — pre-ACP runs |
| `server_verified` | "Legacy (verified)" — pre-ACP runs |
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
12. Persisted `provider-settings.json` with Goose-era values loads correctly after `migrateFromGooseEra()`.
13. `AgentExecution.runtimeSessionID` reads legacy `gooseSessionID` column via `@Attribute(originalName:)`.
14. `proposal-033` gate passes with second-wave ACP prerequisite.

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
