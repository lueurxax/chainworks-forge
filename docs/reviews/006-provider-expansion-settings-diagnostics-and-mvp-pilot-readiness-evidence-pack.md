# Proposal 006 Evidence Pack

| Field | Value |
|---|---|
| Proposal | `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` |
| Prepared At | `2026-03-24T09:01:22+0200` |
| Proposal MD5 | `c5238fbe299bfb7bd21c33fe558fecde` |
| Proposal MTime | `2026-03-24 08:47:38 +0200` |
| Repository SHA | `c62515f` |
| Evidence Completeness | `Partial` |

## Scope

Review target:
- current Proposal 006 draft readiness
- current-head bootstrap/runtime baseline relevant to provider configuration
- current-head app shell evidence relevant to whether Proposal 006 surfaces exist

Out of scope for live review:
- unimplemented Proposal 006 flows
- in-app provider configuration UX
- first-run wizard UX
- diagnostics/preflight UX
- settings transfer UX
- pilot readiness UX

## Evidence Items

### E-P006-001 — Proposal source reread

- File: `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- Key sections reviewed:
  - context and dependency boundary
  - provider registry and health ownership
  - env migration and precedence contract
  - app configuration model
  - settings transfer package
  - first-run wizard
  - diagnostics/preflight
  - file structure
  - acceptance criteria
  - locked decisions

### E-P006-002 — Current bootstrap/runtime configuration path

- Files reviewed:
  - `Chainworks Forge/Chainworks_ForgeApp.swift:168-229`
  - `Chainworks Forge/Engine/ExecutionService.swift:20-53`
- Observed current reality:
  - live runtime is still bootstrapped from environment variables
  - `CHAINWORKS_GOOSE_BASE_URL` remains the network runtime trigger
  - fixture mode is still environment-driven
  - there is no current in-app provider registry or persisted app configuration store

### E-P006-003 — Current workspace contract

- Files reviewed:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:324-342`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:468`
- Observed current reality:
  - `workspaceRoot` is run-scoped under Application Support
  - `artifactRoot` is always derived as `{workspaceRoot}/artifacts`
  - `worktreeRoot` currently remains `nil` in the control-plane baseline

### E-P006-004 — Current app shell and code search baseline

- Files reviewed:
  - `Chainworks Forge/ContentView.swift:4-86`
- Code-search checks:
  - no `ProviderSettingsView`
  - no `FirstRunSetupWizard`
  - no `PreflightReportView`
  - no `PilotReadinessView`
  - no `ProviderRegistry`
  - no `AppConfigurationStore`
  - no `ProviderSettingsStore`
  - no `SettingsTransferService`
  - no `KeychainSecretStore`
  - no `ProviderDiagnosticService`
  - no `BootstrapConfigurationResolver`
- Result:
  - current app shell includes operator-era surfaces from Proposal 005 (`Runs Home`, `Approvals`, `Ideas`, `Agent Catalog`, `Workflow Inspector`)
  - Proposal 006-specific runtime/settings surfaces are still absent

### E-P006-005 — Fresh build on current HEAD

- Command:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal006-r3-build build`
- Result:
  - `passed`
- Derived data:
  - `/tmp/codex-dd-proposal006-r3-build`
- Observed nuance:
  - build is green
  - Swift 6 actor-isolation warnings remain in YAML parsing/validation and several tests, but they did not block this build

### E-P006-006 — Fresh macOS UI-baseline rerun on current HEAD

- Final command:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal006-r3-ui2 test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface'`
- Result:
  - `4/4 passed`
- xcresult:
  - `/tmp/codex-dd-proposal006-r3-ui2/Logs/Test/Test-Chainworks Forge-2026.03.24_08-59-41-+0200.xcresult`
- Observed attachments in this run:
  - `REQ011_Approvals`
  - `P004_NonHappy_MissingRuntime`
  - `REQ011_RunProgress_Entry`
  - `REQ011_RunProgress_Overview`
  - `REQ011_RunProgress_Sections`
  - `REQ011_Sheet`

### Discarded preliminary UI attempt

- Command:
  - `xcodebuild ... -derivedDataPath /tmp/codex-dd-proposal006-r3-ui test -only-testing:'Chainworks ForgeUITests/testApprovalInboxReachable' ...`
- Result:
  - `0 tests executed`
- Why discarded:
  - the filter syntax was incomplete, so this run does not count as real UI evidence

## Attempted But Missing

- No Proposal 006 screens could be reviewed because they do not exist in current app source.
- No current-round provider-settings or wizard attachments can exist until those surfaces are implemented.

## Evidence Gate Assessment

- repo/code inspection: `met`
- build/run attempt logged: `met`
- platform-appropriate UI screenshots/attachments: `met for current baseline shell`
- completed evidence pack with IDs: `met`

Why this still remains `Partial`:
- the baseline evidence is fresh and real
- but the reviewed proposal slice itself is still unimplemented, so the review can only assess draft readiness plus current app baseline, not the actual Proposal 006 operator flow
