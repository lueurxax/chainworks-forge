# Proposal 006: Provider Expansion — Multi-Provider Routing, Settings, Diagnostics, and MVP Pilot Readiness

| Field | Value |
|---|---|
| Date | 2026-03-23 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 001, Proposal 002, Proposal 003, [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md), [005-operator-experience-reports-recovery-and-run-comparison.md](005-operator-experience-reports-recovery-and-run-comparison.md) |
| Goal | Expand the runtime from the fixed MVP provider pair to a real multi-provider surface, add the settings/diagnostics needed to operate it, and make the product ready to be exercised in a serious MVP pilot. |

---

## 1. Context

The original MVP deliberately fixed the provider story to a small baseline so the core control plane could stabilize.
That was the right move.

At this point in the roadmap:
- Proposal 001 gave the catalog model, backend profiles, permission profiles, artifacts, and immutable run snapshots.
- Proposal 002 created the execution engine but explicitly deferred real provider integration, multi-provider routing, settings, and richer provider/runtime concerns.
- Proposal 003 remains adjacent meta-layer work, not the provider/runtime baseline owner.
- [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md) delivered the first real live proposal-loop slice, but intentionally stayed read-only and proposal-only.
- [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md) is the transport follow-on that connects the live slice to a real Goose server runtime.
- [005-operator-experience-reports-recovery-and-run-comparison.md](005-operator-experience-reports-recovery-and-run-comparison.md) made the current baseline calmer to operate, but did not add repo-backed implementation/release capability.

What is still missing before the MVP feels alive “in the wild” is the messy but essential layer around providers:
- more than two backends,
- configuration that survives relaunch,
- secret handling,
- capability-aware backend resolution,
- diagnostics before a run starts,
- support for mixed-provider workflows,
- clear receipts for cost/usage,
- and a first-run/pilot experience that does not require tribal knowledge.

Proposal 006 turns the provider layer into a real product surface.

Important boundary:

- Proposal 006 is about provider/platform readiness for the current control-plane baseline.
- Proposal 006 does **not** assume that repo-backed implementation, writable worktrees, or manual release flows already exist.
- Those capabilities remain owned by Proposal 007.

---

## 2. Target outcome

After Proposal 006, the engineer should be able to:

1. Configure multiple provider families in-app
2. Bind different agents to different providers/models/effort levels
3. Verify provider health before starting a run
4. Start a mixed-provider run with immutable provider bindings captured in the run snapshot
5. See provider-specific receipts normalized into one operator surface
6. Diagnose provider failures without guessing
7. Onboard a new machine and get to a real demo run quickly

This is pilot readiness for the provider/platform layer, not full delivery-slice readiness.
The repo-backed end-to-end dogfood path still belongs to Proposal 007.

---

## 3. What we build

### Layer H: Provider Platform + MVP Readiness

| Component | Responsibility |
|---|---|
| **Provider Registry** | Canonical registry of installed/configured providers and their capabilities |
| **Provider Adapter Layer** | Concrete adapters for supported provider families behind the existing execution boundary |
| **Backend Profile Resolver V2** | Resolve catalog backend profiles against real installed/configured providers |
| **App Configuration Store** | Persist workspace roots, YAML paths, worktree base path, and active configuration source across relaunch |
| **Secrets & Settings Store** | Persist provider settings locally and secrets in Keychain |
| **Diagnostics & Preflight Service** | Verify providers, binaries, auth, model availability, YAML paths, workspace roots, and permissions before run start |
| **Run Start Overrides** | Optional operator-selected overrides for provider/model/effort at run creation time |
| **Usage Receipt Normalizer** | Normalize provider-specific usage/cost receipts into a common shape |
| **Settings Transfer Service** | Export/import secret-safe settings packages distinct from support bundles |
| **First Run / Pilot Kit** | Setup wizard, sample run path, pilot readiness surface, and support bundle export |

---

## 4. Supported provider families

Proposal 006 expands the provider surface to support the families the product actually cares about:

1. **Codex family**
2. **Claude family**
3. **Gemini family**

A family can have one or more transport styles:
- CLI/session-backed
- local runtime bridge
- HTTP API-backed

The app does not need to expose transport complexity to the operator.
The operator sees:
- provider family,
- configured installation/account,
- model,
- effort,
- capabilities,
- verification status.

### 4.1 V1 recommendation

For MVP V1, implement at least one working adapter per family:
- Codex
- Claude
- Gemini

Optional transport variants can arrive later.

---

## 5. Provider Registry

### 5.1 Purpose

The catalog’s `backend_profiles` are abstract until they bind to something real on the operator’s machine/account.

The Provider Registry is the single runtime source of truth for:
- which provider families are available,
- which installations/accounts are configured,
- what capabilities each one supports,
- and whether the configuration is currently healthy.

### 5.2 Core model

```swift
struct ConfiguredProvider: Identifiable, Codable {
    let id: UUID
    let family: ProviderFamily
    let displayName: String
    let transport: ProviderTransport
    let endpoint: String?
    let authMode: ProviderAuthMode
    let defaultModel: String?
    let status: ProviderStatus
    let capabilities: ProviderCapabilities
    let adapterVersion: String
}

enum ProviderFamily: String, Codable {
    case codex
    case claude
    case gemini
}

enum ProviderTransport: String, Codable {
    case cli
    case localBridge
    case httpAPI
}

enum ProviderStatus: String, Codable {
    case unknown
    case healthy
    case degraded
    case unavailable
}
```

### 5.3 Capability model

```swift
struct ProviderCapabilities: Codable {
    var supportsStreaming: Bool
    var supportsTools: Bool
    var supportsStructuredOutput: Bool
    var supportsEffortControl: Bool
    var supportsSessionResume: Bool
    var supportsFileEditing: Bool
    var supportsSandboxHints: Bool
}
```

Capabilities are used in:
- diagnostics,
- backend profile validation,
- run-start warnings,
- and adapter behavior.

### 5.4 Migration and precedence contract

Proposal 006 must collapse the current split truth between env-driven runtime bootstrap and the new in-app provider registry.

Resolution rules:

1. `AppConfigurationStore` and `ProviderSettingsStore` become the canonical runtime inputs after Proposal 006 lands.
2. Legacy environment variables remain supported only as:
   - first-run seeding input,
   - or explicit development override.
3. First-run seeding happens only when persisted settings are absent.
4. Persisted settings do not get silently replaced on every launch by environment variables.
5. Development override requires an explicit flag such as `CHAINWORKS_ALLOW_ENV_OVERRIDE=1`.
6. Diagnostics and preflight must show the active configuration source:
   - `persisted_settings`
   - `seeded_from_env`
   - `development_env_override`

This gives the operator one explainable source of truth while preserving a dev-only escape hatch.

---

## 6. Provider Adapter Layer

### 6.1 Design goal

Do not leak provider-specific behavior into the workflow engine.

The workflow engine continues to speak through an execution boundary.
Proposal 006 upgrades that boundary with provider-aware receipts, capability validation, and richer diagnostics.

### 6.2 Adapter responsibilities

Each adapter must:
- verify installation/auth,
- expose available or configured models,
- execute an agent task,
- stream status/log/progress events,
- return normalized usage/cost metadata,
- provide actionable error taxonomy.

### 6.3 Common execution receipt

```swift
struct ProviderExecutionReceipt: Sendable, Codable {
    let providerFamily: String
    let configuredProviderID: UUID
    let model: String
    let effort: String?
    let transport: String
    let inputTokens: Int?
    let outputTokens: Int?
    let billedUnits: Int?
    let costCents: Int64?
    let wallClockSeconds: Double
    let rawReceiptJSON: Data?
}
```

### 6.4 Failure taxonomy

```swift
enum ProviderExecutionError: Error {
    case authFailed(String)
    case modelUnavailable(String)
    case capabilityMismatch(String)
    case transportUnavailable(String)
    case rateLimited(String)
    case timeout(String)
    case executionFailed(String)
}
```

This error taxonomy is the basis for:
- preflight blocking,
- run blocked reasons,
- operator recovery suggestions,
- support bundle export.

---

## 7. Backend Profile Resolver V2

### 7.1 Why

`backend_profiles` in `agents.yaml` are part of the static catalog.
Operators also need:
- installed provider inventory,
- machine-local defaults,
- optional run-start overrides,
- capability validation,
- and immutable binding captured in the run.

### 7.2 Resolution order

When an agent is bound for a specific run:

1. catalog backend profile
2. optional run-start override
3. resolved configured provider installation/account
4. capability validation
5. immutable run binding snapshot

### 7.3 Immutable provider binding snapshot

A run must not depend on mutable app settings after it starts.

Proposal 006 adds provider binding snapshot data to the run:

```swift
// Added to Run
var providerBindingSnapshotJSON: Data?   // resolved provider IDs/models/transports/adapter versions
var startOptionsJSON: Data?              // run-start overrides and operator choices
```

This snapshot records:
- which configured provider instance was selected,
- which model,
- which transport,
- which effort,
- adapter version,
- any operator override applied.

### 7.4 No silent fallback

Default policy:
- no automatic provider substitution mid-run,
- no silent model fallback,
- no hidden effort downgrades.

If a binding cannot be satisfied:
- block before run start, or
- fail/block the run with explicit reason.

Fallback, when supported later, must be:
- explicit,
- visible,
- recorded in the run snapshot or recovery log.

---

## 8. Settings, secrets, and onboarding

### 8.1 Settings split

Proposal 006 has two persisted non-secret stores plus Keychain:

1. `AppConfigurationStore`
2. `ProviderSettingsStore`
3. `KeychainSecretStore`

`AppConfigurationStore` owns machine-local runtime inputs that are not specific to one provider account.
`ProviderSettingsStore` owns configured providers and provider preferences.
Secrets remain in Keychain only.

### 8.2 App configuration source of truth

```swift
struct AppConfiguration: Codable {
    var workspaceRootPath: String
    var artifactBasePath: String
    var worktreeBasePath: String?
    var workflowSourcePath: String
    var agentCatalogSourcePath: String
    var supportBundleExportPath: String?
    var activeConfigurationSource: ConfigurationSource
}

enum ConfigurationSource: String, Codable {
    case persistedSettings
    case seededFromEnv
    case developmentEnvOverride
}
```

This model is owned by:

- `Support/AppConfiguration.swift`
- `Support/AppConfigurationStore.swift`
- `Support/BootstrapConfigurationResolver.swift`

It is consumed by:

- app bootstrap,
- first-run wizard,
- preflight,
- run-start compilation,
- pilot readiness screen.

### 8.3 Provider settings

```swift
struct ProviderSettings: Codable {
    var configuredProviders: [ConfiguredProvider]
    var preferredProviderIDsByFamily: [String: UUID]
    var notificationOnProviderFailure: Bool
    var runStartRequiresCleanPreflight: Bool
}
```

### 8.4 Secrets

Examples:
- API keys
- session tokens
- CLI auth handles if needed
- endpoint credentials

Key principles:
- never export secrets in support bundles,
- never store secrets in YAML,
- never include secrets in run report artifacts.

### 8.5 Settings import / export

Proposal 006 keeps settings import/export in scope, but it must be a real component, not an acceptance-criteria ghost.

Component:

- `Support/SettingsTransferService.swift`

Exported file:

- `chainworks-settings.json`

Payload shape:

```swift
struct ExportableSettingsPackage: Codable {
    var appConfiguration: AppConfiguration
    var providerSettings: ProviderSettings
    var exportedAt: Date
    var appVersion: String
    var secretPlaceholders: [String]
}
```

Rules:

- secrets are never exported,
- exported provider entries that require secrets include placeholders only,
- import validates file version, required paths, provider families, and placeholder completeness,
- import never overwrites secrets silently,
- import produces an actionable remediation list for missing credentials.

UI entry points:

- `ProviderSettingsView`
- `FirstRunSetupWizard`

Support bundle export remains a separate diagnostics artifact and is not the same thing as settings transfer.

### 8.6 First Run Setup Wizard

The wizard guides the operator through:
1. choosing workspace roots / YAML locations,
2. configuring providers,
3. verifying providers,
4. loading agent/workflow catalog,
5. running a sample workflow.

The wizard must persist:

- `AppConfiguration` via `AppConfigurationStore`
- `ProviderSettings` via `ProviderSettingsStore`

The wizard does not own ad hoc runtime-only path state.

This matters more than it sounds.
Without it, the MVP remains “good if you already know the system.”

---

## 9. Diagnostics & Preflight Service

### 9.1 Why

The operator should find out *before* hitting Start Run that:
- a provider is missing,
- auth is broken,
- a model is unavailable,
- YAML paths are wrong,
- or workspace configuration is invalid.

### 9.2 Preflight categories

1. **Catalog**
   - active configuration source visible
   - workflow file exists
   - agent catalog exists
   - YAML parses and validates

2. **Providers**
   - configured providers exist
   - required families satisfied
   - auth works
   - selected/default models resolvable

3. **Workspace**
   - workspace root exists
   - artifact root writable
   - worktree base path valid

4. **Permissions**
   - required runtime tools available
   - expected side-effect services reachable
   - no workspace isolation contract violations detected in config

5. **Environment**
   - required env vars present if referenced
   - app support dirs writable
   - keychain accessible

### 9.3 Result model

```swift
struct PreflightReport: Codable {
    let status: PreflightStatus
    let configurationSource: ConfigurationSource
    let checks: [PreflightCheck]
    let warnings: [String]
    let blockingIssues: [String]
}

enum PreflightStatus: String, Codable {
    case pass
    case warn
    case fail
}
```

### 9.4 Start Run gate

If preflight status is:
- `pass` → start enabled
- `warn` → start allowed with explicit confirmation
- `fail` → start disabled

Preflight is evaluated against `AppConfigurationStore` plus `ProviderSettingsStore`, not against ad hoc path parameters.

---

## 10. Run Start Overrides

### 10.1 Why

Operators need a controlled way to answer questions like:
- “Use Gemini for UX review in this run only.”
- “Temporarily raise effort for the architect.”
- “Swap proposal writer model for a test run.”

### 10.2 Scope

Run-start overrides are allowed only for:
- provider family / configured provider selection
- model
- effort

They are **not** allowed for:
- permission profile
- workspace policy
- agent prompt
- skill ref
- output contracts

### 10.3 Persistence

Overrides are captured in `Run.startOptionsJSON` and in the provider binding snapshot.
They do not mutate the catalog.

---

## 11. Usage receipt normalization

### 11.1 Why

Different providers expose different usage units and billing shapes.
The operator needs one place to reason about cost and resource usage.

### 11.2 Common surface

Proposal 006 standardizes:
- provider family
- configured provider
- model
- effort
- latency
- cost in cents when available
- raw receipt payload stored for debugging

### 11.3 Model additions

```swift
// Added to AgentExecution
var providerReceiptJSON: Data?
var resolvedModel: String?
var configuredProviderID: UUID?
var adapterVersion: String?
```

This complements the existing provider/model/effort fields with an auditable receipt.

---

## 12. MVP Pilot Kit

### 12.1 Goal

Make the MVP easy to exercise as a product, not just as a codebase.

### 12.2 Components

#### Setup Wizard
Gets a fresh machine from zero to “ready to run.”

#### Sample Run Path
A guided path that:
- creates a sample idea,
- selects a known-good workflow,
- runs a proposal-loop or other explicitly read-only safe slice,
- surfaces report and artifacts.

#### Support Bundle Export
Produces a zip containing:
- run reports,
- selected logs,
- provider verification summary,
- configuration summary without secrets,
- artifact index,
- app version,
- adapter versions.

#### Pilot Readiness Screen
One place to see:
- provider health,
- YAML health,
- workspace health,
- active configuration source,
- last successful run,
- blocked runs,
- pending approvals.

This is a high-leverage “feel” feature.
It makes the MVP much easier to trust and demo.

---

## 13. File structure

```
Chainworks Forge/
  Providers/
    ConfiguredProvider.swift              ← NEW
    ProviderSettings.swift                ← NEW
    ProviderSettingsStore.swift           ← NEW
    ProviderRegistry.swift                ← NEW
    ProviderCapabilities.swift            ← NEW
    ProviderAdapter.swift                 ← NEW
    CodexProviderAdapter.swift            ← NEW
    ClaudeProviderAdapter.swift           ← NEW
    GeminiProviderAdapter.swift           ← NEW
    BackendProfileResolverV2.swift        ← NEW
    ProviderExecutionReceipt.swift        ← NEW
    ProviderDiagnosticService.swift       ← NEW
    UsageReceiptNormalizer.swift          ← NEW
    KeychainSecretStore.swift             ← NEW

  Engine/
    PreflightService.swift                ← NEW
    RunStartOverrideResolver.swift        ← NEW
    SupportBundleExporter.swift           ← NEW

  Models/
    Run.swift                             ← CHANGED: providerBindingSnapshotJSON, startOptionsJSON
    AgentExecution.swift                  ← CHANGED: providerReceiptJSON, resolvedModel, configuredProviderID, adapterVersion

  Support/
    AppConfiguration.swift                ← NEW
    AppConfigurationStore.swift           ← NEW
    BootstrapConfigurationResolver.swift  ← NEW
    SettingsTransferService.swift         ← NEW

  Views/
    ProviderSettingsView.swift            ← NEW
    FirstRunSetupWizard.swift             ← NEW
    PreflightReportView.swift             ← NEW
    PilotReadinessView.swift              ← NEW
    RunStartOverridesView.swift           ← NEW
```

---

## 14. Acceptance criteria

### Provider expansion
- [ ] App can configure at least Codex, Claude, and Gemini provider families
- [ ] A workflow can execute with mixed providers across agents
- [ ] Provider capabilities are visible in settings/diagnostics
- [ ] Provider binding is frozen into the run snapshot at start time

### Settings & secrets
- [ ] `AppConfiguration` persists workspace root, YAML paths, and worktree base path across relaunch
- [ ] Non-secret provider settings persist across relaunch
- [ ] Secrets are stored in Keychain, not in YAML or plain app storage
- [ ] Settings export/import exists as a dedicated path and excludes secrets cleanly
- [ ] Diagnostics and preflight show the active configuration source

### Diagnostics & preflight
- [ ] Preflight checks run before start
- [ ] Missing auth, missing provider, or invalid model blocks run start
- [ ] Warn-level preflight issues require explicit confirmation
- [ ] Pilot readiness screen shows provider/YAML/workspace status

### Overrides
- [ ] Operator can override provider/model/effort at run start
- [ ] Override is recorded in the run snapshot
- [ ] Overrides do not mutate catalog YAML

### Usage receipts
- [ ] Each provider-backed `AgentExecution` stores a normalized receipt
- [ ] Cost/latency are visible in run/operator surfaces
- [ ] Raw provider receipt remains available for debugging

### MVP pilot readiness
- [ ] Fresh-machine setup wizard can get the app to “ready to run”
- [ ] Sample run path executes a provider-safe read-only workflow successfully after setup
- [ ] Support bundle export produces a secret-safe zip
- [ ] Operator can diagnose provider failure without opening source code

### General
- [ ] No regressions in Proposal 001, Proposal 002, Proposal 003, [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [005-goose-server-transport-adapter.md](005-goose-server-transport-adapter.md), and [005-operator-experience-reports-recovery-and-run-comparison.md](005-operator-experience-reports-recovery-and-run-comparison.md)
- [ ] `xcodebuild build && xcodebuild test` green

### Product checkpoint (PROD-PA-006)
- [ ] A new machine can be configured and run a mixed-provider read-only workflow in under 15 minutes
- [ ] A blocked provider setup can be diagnosed from in-app surfaces without inspecting raw logs manually
- [ ] The MVP can be demoed with at least three provider families and produce auditable receipts per agent execution

---

## 15. What's NOT in scope

| Exclusion | Reason |
|---|---|
| Automatic provider benchmarking / AB routing | Useful later, but too much surface for MVP readiness |
| Silent runtime fallback chains | Dangerous for reproducibility and debugging |
| Team / cloud provider administration | Product is still local-first |
| Temporal migration / distributed workers | Separate architectural phase |
| Automatic prompt retuning based on provider performance | Steward/meta-layer work, not provider readiness |
| Provider pricing scraper / live pricing sync | Static/manual mapping is enough for MVP |

---

## 16. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| ARCH-061 | Run provider bindings are immutable once the run starts | Reproducibility and trustworthy comparison |
| ARCH-062 | No silent provider/model fallback in V1 | Hidden substitution destroys trust |
| ARCH-063 | Secrets live in Keychain only | Basic security hygiene and support-bundle safety |
| ARCH-064 | Preflight is mandatory before starting a real run | Catch provider/config failures before execution |
| ARCH-065 | Run-start overrides are narrow and explicit | Enough flexibility without turning the app into an ad hoc config editor |
| ARCH-066 | Provider receipts are normalized but raw receipts are retained | Operators need both a common surface and debug truth |
| ARCH-067 | `AppConfigurationStore` plus `ProviderSettingsStore` become the canonical runtime source of truth | Avoid split configuration truth between env vars and in-app settings |
| ARCH-068 | Environment variables are seeding or explicit dev override inputs only | Preserve developer ergonomics without silent runtime drift |
| ARCH-069 | Proposal 006 pilot readiness is provider/platform readiness, not repo-backed delivery readiness | Keep Proposal 007 as the owner of writable implementation/release flows |

---

## 17. Execution plan

| Day | Deliverable |
|---|---|
| Day 1 | Provider registry + capability model + settings data model |
| Day 2 | Keychain secret store + provider settings UI |
| Day 3 | Provider adapters for Codex / Claude / Gemini families |
| Day 4 | BackendProfileResolver V2 + run-start overrides |
| Day 5 | Preflight service + diagnostics UI |
| Day 6 | Usage receipt normalization + model persistence |
| Day 7 | First Run Setup Wizard + Sample Run path |
| Day 8 | Pilot Readiness screen + support bundle export |
| Day 9 | Cross-provider smoke tests + polish |

---

## 18. What this proposal enables

Proposal 006 is the point where Chainworks starts to feel like a real MVP instead of an internal prototype.

It enables:
- real backend diversity,
- machine-local configuration that survives real use,
- safer provider operation,
- faster setup on new machines,
- and a much more credible pilot/demo experience.

In other words:
the control plane can finally be exercised the way it was originally intended — with different agents riding different backends, under explicit settings, with receipts and diagnostics that make the system trustworthy.
