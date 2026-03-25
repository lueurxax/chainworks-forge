# Proposal 006: Provider Expansion — Multi-Provider Routing, Settings, Diagnostics, and MVP Pilot Readiness Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` |
| Repository Root | `.` |
| Git SHA | `62357bb` |
| Working Tree | dirty |
| Audited At | `2026-03-25T08:08:44+02:00` |
| Proposal State | Active |
| Overall Status | Not Implemented |

## Verdict

Proposal 006 is largely implemented in the current repository: the provider registry, app/settings stores, Keychain-backed secret storage, settings transfer, mixed-provider resolution, preflight, sample-run launcher, pilot-readiness surfaces, and support-bundle export all exist in code. The proposal is still not fully implemented because the current Proposal 006 UI proof is not green and the proposal's own general acceptance gate (`xcodebuild build && xcodebuild test`) is not closed by this audit. In the focused Proposal 006 test pass, `testProviderSettingsWizardFlowSurface`, `testProviderSettingsExportSurface`, and `testPilotReadinessRefreshSurface` failed on direct-surface bootstrap, so MVP-readiness sign-off remains open.

## Proposal Contract

### Scope

- Add a provider platform for Codex, Claude, and Gemini with a canonical Provider Registry.
- Persist machine-local app configuration separately from provider settings and secrets.
- Add diagnostics/preflight, limited run-start overrides, normalized usage receipts, and an MVP pilot kit.
- Keep Proposal 006 provider/platform-focused; repo-backed delivery belongs to later work.

### Locked Decisions

- `ARCH-061`: provider bindings are frozen per run.
- `ARCH-062`: no silent provider fallback.
- `ARCH-063`: secrets live in Keychain only.
- `ARCH-064`: preflight is mandatory before run start.
- `ARCH-065`: overrides are limited to provider/model/effort.
- `ARCH-066`: preserve both normalized and raw receipts.
- `ARCH-067`: `AppConfigurationStore` + `ProviderSettingsStore` are canonical runtime truth.
- `ARCH-068`: environment variables are seeding/dev override only.
- `ARCH-069`: Proposal 006 is not repo-backed delivery readiness.
- `ARCH-070`: preserve existing workspace/artifact-root contract.
- `ARCH-071`: provider health is derived runtime state, not persisted truth.

### Acceptance Criteria

- Configure Codex, Claude, and Gemini providers and resolve mixed-provider workflows.
- Persist workflow/catalog/workspace paths across relaunch.
- Persist non-secret provider settings, keep secrets in Keychain, and support export/import without secrets.
- Surface diagnostics/preflight with active configuration source and fail/warn gating.
- Freeze provider/model/effort bindings per run and persist overrides in run snapshots.
- Store normalized provider receipts plus raw receipts.
- Expose first-run wizard, pilot-readiness, sample run path, and support bundle export.
- Keep the tree green under `xcodebuild build && xcodebuild test`.
- Support a product-checkpoint demo for fresh-machine readiness and in-app diagnosis.

### Test / Evidence Requirements

- Repository proof through focused code inspection.
- Apple-platform proof through `xcodebuild` build/test.
- Runtime/UI proof for settings, onboarding, readiness, and operator surfaces.

### Explicit Exclusions

- Repo-backed implementation/release workflows.
- Broad writable delivery infrastructure.
- Silent fallback or secret persistence in exported settings.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 5 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Canonical app configuration persists machine-local paths and source precedence
- Proposal Source: `§8.2 App configuration source of truth` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:367-410`), `§14 Settings & secrets` (`:737-743`), `ARCH-067/068` (`:797-804`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Support/AppConfiguration.swift:3-75`
  - `Chainworks Forge/Support/AppConfigurationStore.swift:4-74`
  - `Chainworks Forge/Support/BootstrapConfigurationResolver.swift:3-45`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:138-167`
- Gap / Note: The precedence contract is explicit: persisted settings win unless `CHAINWORKS_ALLOW_ENV_OVERRIDE=1`, and serialized configuration uses canonical snake_case wire values.

### REQ-002 Provider settings persist durable configuration while provider health is derived at runtime
- Proposal Source: `§5 Provider Registry` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:115-232`), `§8.3 Provider settings` (`:412-425`), `ARCH-071` (`:805`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-130`
  - `Chainworks Forge/Providers/ProviderSettings.swift:3-14`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:4-137`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:4-62`
- Gap / Note: The old design risk is closed in code: persisted settings contain durable provider configuration only, while `latestHealthByProviderID` and `lastRefreshedAt` are runtime-owned.

### REQ-003 Secrets are stored in Keychain only
- Proposal Source: `§8.4 Secrets` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:426-438`), `ARCH-063` (`:799`)
- Status: Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/KeychainSecretStore.swift:18-108`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:264-269`
- Gap / Note: Tests use the in-memory test store, but production storage is Keychain-backed and the settings stores do not persist secrets.

### REQ-004 Settings transfer exports/imports non-secret settings with schema and placeholder validation
- Proposal Source: `§8.5 Settings import / export` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:439-485`), `§14 Settings & secrets` (`:737-743`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Support/SettingsTransferService.swift:3-119`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:269-317`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:319-401`
- Gap / Note: The transfer package now includes `transferSchemaVersion`, excludes secret values, and fails closed when required placeholders are unresolved.

### REQ-005 Backend Profile Resolver V2 supports mixed-provider routing with no silent fallback
- Proposal Source: `§7 Backend Profile Resolver V2` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:293-352`), `ARCH-061/062` (`:793-799`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:3-81`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:169-221`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:223-267`
- Gap / Note: Resolution now binds to configured providers or explicit overrides and throws for missing provider/model instead of falling back silently.

### REQ-006 Provider bindings are frozen into the run snapshot
- Proposal Source: `§7.3 Immutable provider binding snapshot` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:315-334`), `§10.3 Persistence` (`:600-606`), `§14 Provider expansion / Overrides` (`:731-755`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Models/Run.swift:39-45`
  - `Chainworks Forge/Engine/SampleRunLauncher.swift:33-58`
  - `Chainworks Forge/Views/IdeaListView.swift:858-867`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:542-621`
- Gap / Note: The launcher test proves both `providerBindingSnapshotJSON` and `startOptionsJSON` are written.

### REQ-007 Diagnostics and preflight surface active configuration source and enforce pass/warn/fail gating
- Proposal Source: `§9 Diagnostics & Preflight Service` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:507-576`), `§14 Diagnostics & preflight` (`:745-750`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:3-332`
  - `Chainworks Forge/Views/PreflightReportView.swift:3-65`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:398-540`
- Gap / Note: The test slice covers missing provider families, missing credentials, and invalid override models as blocking failures.

### REQ-008 Run-start overrides are limited to provider, model, and effort, and persisted with the run
- Proposal Source: `§10 Run Start Overrides` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:577-606`), `ARCH-065` (`:801`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Views/RunStartOverridesView.swift:3-96`
  - `Chainworks Forge/Engine/RunStartOverrideResolver.swift:3-38`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49-77`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:169-221`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:542-621`
- Gap / Note: The surfaced override UI and resolver only expose/provider-model-effort changes; no broader catalog mutation path is present.

### REQ-009 Agent executions persist normalized provider metadata and receipts
- Proposal Source: `§6.3 Common execution receipt` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:253-270`), `§11 Usage receipt normalization` (`:607-638`), `§14 Usage receipts` (`:756-760`)
- Status: Partially Implemented
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/ProviderExecutionReceipt.swift:3-15`
  - `Chainworks Forge/Providers/UsageReceiptNormalizer.swift:3-27`
  - `Chainworks Forge/Models/AgentExecution.swift:18-31`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:462-467`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:628-632`
- Gap / Note: The persistence path is present, but this audit did not close a fresh operator-facing proof that normalized receipt data plus raw receipt payload are surfaced end-to-end from a completed provider run.

### REQ-010 First Run Setup Wizard exists for workspace/YAML/provider setup, transfer actions, preflight, and sample-run launch
- Proposal Source: `§8.6 First Run Setup Wizard` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:486-506`), `§12 MVP Pilot Kit` (`:641-682`), `§14 MVP pilot readiness` (`:761-766`)
- Status: Partially Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:4-303`
  - `Chainworks Forge/Views/ProviderSettingsView.swift:24-56`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-targeted.SUnVYn -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface' test`
- Gap / Note: The wizard is implemented, but the current focused UI proof fails: `testProviderSettingsWizardFlowSurface` fails at `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:288` with `First run setup direct surface must finish bootstrap`.

### REQ-011 Provider Settings and Pilot Readiness operator surfaces are live and usable
- Proposal Source: `§12 MVP Pilot Kit` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:641-682`), `§14 MVP pilot readiness` (`:761-766`)
- Status: Partially Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:4-319`
  - `Chainworks Forge/Views/PilotReadinessView.swift:4-288`
  - `Chainworks Forge/ContentView.swift:63-127`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:242-278`
  - `xcodebuild ... -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface' test`
  - `xcodebuild ... -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' test`
- Gap / Note: The tab-entry surfaces pass, but deeper Proposal 006 operator proofs are still red. `testProviderSettingsExportSurface` fails at `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:311` and `testPilotReadinessRefreshSurface` fails at `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:334`, both before the direct surfaces finish bootstrap.

### REQ-012 Support bundle export produces a secret-safe diagnostic bundle
- Proposal Source: `§12.2 Components` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:645-682`), `§14 MVP pilot readiness` (`:761-766`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Engine/SupportBundleExporter.swift:5-240`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:623-718`
  - `Chainworks Forge/Views/PilotReadinessView.swift:215-225`
- Gap / Note: The audited test proves archive creation plus the expected diagnostic payloads and selected artifacts.

### REQ-013 Sample run path launches the provider-safe workflow and freezes provider bindings
- Proposal Source: `§12.2 Components` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:645-682`), `§14 MVP pilot readiness` (`:761-766`)
- Status: Implemented
- Evidence Type: `code, tests-run`
- Evidence:
  - `Chainworks Forge/Engine/SampleRunLauncher.swift:11-58`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift:142-159`
  - `Chainworks Forge/Views/PilotReadinessView.swift:144-161`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:542-621`
- Gap / Note: The focused test closes the proposal-safe launcher path in code, though this audit did not run a live real-provider sample from the GUI.

### REQ-014 General sign-off gate stays green under `xcodebuild build && xcodebuild test`
- Proposal Source: `§14 General` (`docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md:767-770`)
- Status: Missing
- Evidence Type: `tests-run`
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-build.awiI2s build` (`BUILD SUCCEEDED`)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-targeted.SUnVYn -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' test`
- Gap / Note: The focused Proposal 006 test slice is red. Current failures include `testProviderSettingsWizardFlowSurface`, `testProviderSettingsExportSurface`, and `testPilotReadinessRefreshSurface`. A full-scheme `xcodebuild test` attempt was also started under `/tmp/codex-p006-audit-full.kHqBJ1`, but the audit did not obtain a clean green completion from that run.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- `stat -f '%Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- `rg -n "ProviderSettingsView|PilotReadinessView|FirstRunSetupWizard|PreflightService|SettingsTransferService|ProviderRegistry|AppConfigurationStore" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-build.awiI2s build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-targeted.SUnVYn -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' test`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p006-audit-full.kHqBJ1 test` (attempt started; no green sign-off captured in this audit)

## Recommended Next Actions

- Fix the direct-surface bootstrap path used by `ProviderSettingsView`, `PilotReadinessView`, and `FirstRunSetupWizard` so the current UI proof closes cleanly.
- Rerun the focused Proposal 006 UI slice until `testProviderSettingsWizardFlowSurface`, `testProviderSettingsExportSurface`, and `testPilotReadinessRefreshSurface` are green.
- After the UI slice is green, rerun a full-scheme `xcodebuild test` and keep the result bundle path as the final sign-off artifact for `REQ-014`.
