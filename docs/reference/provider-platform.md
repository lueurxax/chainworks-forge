# Provider Platform

Stable reference for the multi-provider, settings, diagnostics, and pilot-readiness baseline.

## Purpose

The provider layer is part of the product, not hidden glue.

The app must be able to:

- configure real provider families,
- persist machine-local runtime settings,
- validate the environment before run start,
- freeze provider bindings into the run snapshot,
- normalize receipts and costs,
- and onboard a fresh machine without tribal knowledge.

This document records that implemented baseline as a reference contract.

Related stable docs:

- [provider-binding-truth.md](provider-binding-truth.md)
- [project-workspace-contract.md](project-workspace-contract.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)

## Supported MVP provider families

The current MVP provider set is normalized into five canonical families for 
consistent capacity management and scheduling:

1. `claude` (aliases: `claude_acp`, `claude_agent`, `claude_agent_acp`)
2. `gemini` (aliases: `gemini_acp`, `gemini_cli`, `gemini_cli_acp`)
3. `codex` (aliases: `codex_acp`, `codex_cli`, `codex_cli_acp`, `openai_codex`)
4. `auggie` (aliases: `auggie_acp`)
5. `junie` (aliases: `junie_acp`)

### Capacity Caps

The control-plane daemon enforces default active-execution caps per provider family to 
prevent saturation and ensure scheduling fairness:

| Family | Default Cap |
|---|---|
| `claude` | 8 |
| `gemini` | 4 |
| `codex` | 10 |
| `auggie` | 1 |
| `junie` | 1 |

System-wide, the daemon enforces a global cap of **20 active agent executions** and 
a per-run cap of **4 active agent executions**. Surplus work remains queued 
(backpressured) rather than failing.

The operator should reason about provider family, configured installation/account, model, effort, capabilities, and current health without needing to care about transport internals.

Runtime transport selection and per-agent MCP policy are adjacent contracts, but they are not owned here:

- transport-family and ACP adapter truth live in [acp-runtime-transport.md](acp-runtime-transport.md),
- per-agent MCP policy and requested/predicted/actual runtime truth live in [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md).

## Core components

The provider-platform baseline consists of:

| Component | Responsibility |
|---|---|
| `ProviderRegistry` | In-memory runtime registry of configured providers and their latest derived health |
| `ProviderAdapter` layer | Provider-family execution adapters behind the engine boundary |
| `BackendProfileResolverV2` | Resolve catalog backend profiles against configured providers and overrides |
| `AppConfigurationStore` | Persist non-secret machine-local runtime configuration |
| `ProviderSettingsStore` | Persist configured providers and provider preferences |
| `KeychainSecretStore` | Persist secrets outside plain app storage |
| `PreflightService` / diagnostics | Validate provider/platform/workspace readiness before start |
| `SettingsTransferService` | Import/export secret-safe settings packages |
| `SupportBundleExporter` | Export diagnostic bundles distinct from settings transfer |
| `FirstRunSetupWizard` | Guided machine bootstrap |
| `PilotReadinessView` | Operator-facing health summary for the provider/platform slice |

## Durable configuration model

Two persisted non-secret stores plus Keychain form the configuration baseline:

1. `AppConfigurationStore`
2. `ProviderSettingsStore`
3. `KeychainSecretStore`

`AppConfiguration` owns:

- `runStorageBasePath`
- `worktreeBasePath`
- `workflowSourcePath`
- `agentCatalogSourcePath`
- `supportBundleExportPath`
- `activeConfigurationSource`

`ProviderSettings` owns:

- configured providers,
- preferred providers by family,
- notification preferences,
- preflight policy.

Secrets never live in YAML or plain app storage.

## Configuration precedence

After the provider-platform baseline lands, persisted settings become the canonical runtime source of truth.

Environment variables are allowed only as:

- first-run seeding,
- or explicit development override.

The serialized configuration sources are:

- `persisted_settings`
- `seeded_from_env`
- `development_env_override`

Diagnostics and preflight must show which source is active.

## Provider registry and health

`ConfiguredProvider` is durable configuration.
`ProviderHealthSnapshot` is derived runtime state.

Health is refreshed:

- on app launch,
- on explicit diagnostics runs,
- before preflight-backed run start,
- after provider settings change.

Health is not treated as persisted truth across relaunch.

The capability model covers:

- streaming,
- tools,
- structured output,
- effort control,
- session resume,
- file editing,
- sandbox hints.

Those capabilities are consumed by:

- diagnostics,
- backend profile validation,
- run-start warnings,
- adapter behavior.

## Immutable provider bindings

Provider choice must freeze at run start.

Each run stores provider binding snapshot data such as:

- configured provider instance,
- provider family,
- model,
- transport,
- effort,
- adapter version,
- operator override data.

`Run.startOptionsJSON` records run-start overrides.
`Run.providerBindingSnapshotJSON` records the resolved immutable binding.

Default rule:

- no silent provider substitution,
- no hidden model fallback,
- no silent effort downgrade.

If the requested binding cannot be satisfied, the system must block or fail explicitly rather than improvising.

## Run-start overrides

Overrides are narrow on purpose.

Allowed:

- provider family or configured provider selection,
- model,
- effort.

Not allowed:

- permission profile,
- workspace policy,
- agent prompt,
- skill reference,
- output contract.

Overrides apply to the current run only and never mutate YAML/catalog definitions.

## Preflight and diagnostics

The operator should learn about problems before hitting `Start Run`.

Preflight categories:

1. Catalog
2. Providers
3. Workspace
4. Permissions
5. Environment

`PreflightReport` exposes:

- overall status,
- configuration source,
- checks,
- warnings,
- blocking issues.

Run-start gate semantics:

- `pass` -> start enabled
- `warn` -> start allowed only with explicit confirmation
- `fail` -> start disabled

Preflight reads from persisted configuration stores, not ad hoc path state.

Preflight consumes ACP-era adapter health, persisted provider configuration, and runtime compatibility truth directly from the provider platform and runtime layers.

## Usage receipts

Providers expose different billing and usage shapes, so the app normalizes them into a common receipt surface.

Per provider-backed `AgentExecution`, the system records:

- provider family,
- configured provider,
- model,
- effort,
- transport,
- token or billed-unit counts when available,
- cost in cents when available,
- latency,
- raw receipt payload for debugging.

This makes cost and performance visible in operator surfaces without discarding provider-specific truth.

## Settings transfer and support bundles

Settings transfer and support export are separate products.

Settings transfer:

- uses `chainworks-settings.json`,
- includes `transferSchemaVersion`,
- includes app configuration and provider settings,
- never exports secrets,
- carries secret placeholders and remediation on import.

Support bundles:

- are diagnostic exports,
- can include reports, logs, provider verification summary, configuration summary without secrets, artifact index, app version, and adapter versions,
- are not interchangeable with settings transfer.

## First-run and pilot-readiness surfaces

The provider-platform baseline includes:

- `FirstRunSetupWizard`
- `ProviderSettingsView`
- `PreflightReportView`
- `PilotReadinessView`
- `ProviderSetupEvidencePanel`

Those surfaces cover:

- workspace/YAML selection,
- provider configuration,
- provider verification,
- ACP runtime readiness and provider diagnostics,
- sample-run bootstrap,
- current provider/YAML/workspace health,
- configuration source,
- last successful run,
- blocked runs,
- pending approvals.

The goal is to make the system operable on a fresh machine, not just understandable to someone already living in the codebase.
## Boundaries

This reference intentionally stops at provider/platform readiness.

It does not define:

- repo-backed delivery configuration,
- writable worktree ownership,
- git/release side-effect execution,
- delivery-specific recovery.

Those remain the responsibility of [full-mvp-delivery.md](full-mvp-delivery.md).
