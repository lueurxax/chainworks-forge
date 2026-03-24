# Proposal 006 Evidence Pack

| Field | Value |
|---|---|
| Proposal | `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` |
| Prepared At | `2026-03-23T23:26:09+0200` |
| Proposal MD5 | `faf3a79190cd7661160e7fd5df2d9c64` |
| Proposal MTime | `2026-03-23 21:35:02 +0200` |
| Repository SHA | `59b28ea` |
| Evidence Completeness | `Partial` |

## Scope

Review target:
- Proposal 006 draft readiness
- current-head provider/runtime baseline
- current-head app shell evidence relevant to settings/diagnostics/pilot-readiness claims

Out of scope for live review:
- unimplemented Proposal 006 screens and flows
- real multi-provider in-app configuration
- Keychain-backed settings UX
- support bundle export UX

## Evidence Items

### E-P006-001 — Proposal source

- File: `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- Key sections reviewed:
  - context and dependency baseline
  - provider registry and settings model
  - first-run wizard
  - diagnostics/preflight
  - pilot kit
  - file structure
  - acceptance criteria

### E-P006-002 — Roadmap dependency cross-check

- File: `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- Purpose: verify whether implementation/release slices are already part of Proposal 006’s dependency baseline
- Result: Proposal 007 still owns the repo-backed delivery slice

### E-P006-003 — Current runtime configuration code

- Files reviewed:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/ContentView.swift`
- Observed current reality:
  - live runtime is bootstrapped from env vars or fixture env only
  - current app shell tabs are `Ideas`, `Approvals`, `Agent Catalog`, `Workflow Inspector`
  - Start Run UI still explains live runtime setup through env-based recovery copy

### E-P006-004 — Absence of Proposal 006 implementation surfaces

- Search terms checked:
  - `ProviderRegistry`
  - `ConfiguredProvider`
  - `ProviderSettingsView`
  - `FirstRunSetupWizard`
  - `PreflightService`
  - `PreflightReport`
  - `PilotReadinessView`
  - `SupportBundleExporter`
  - `RunStartOverrideResolver`
  - `KeychainSecretStore`
  - `BackendProfileResolverV2`
  - `UsageReceiptNormalizer`
- Result:
  - no matching implementation files/classes found in current app source

### E-P006-005 — Fresh build and targeted test baseline

- Build:
  - command: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' build`
  - result: passed

- Targeted tests:
  - command: `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal006-r1 -only-testing:'Chainworks ForgeTests/GooseAgentExecutorTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable'`
  - result: passed
  - xcresult: `/tmp/codex-dd-proposal006-r1/Logs/Test/Test-Chainworks Forge-2026.03.23_23-23-35-+0200.xcresult`
  - xcresult mtime: `2026-03-23 23:24:47 +0200`

### E-P006-006 — Fresh UI attachments

- Exported from current-head xcresult into:
  - `docs/reviews/artifacts/proposal-006-provider-r1-ui`
- Attachment mapping:
  - `REQ011_Approvals` → `96E4E35E-F648-492B-A408-B63293687B02.png`
  - `P004_NonHappy_MissingRuntime` → `E95706F9-4816-42E1-B3D9-1C81FA80DFF2.png`
  - `REQ011_Sheet` → `4B1BB70F-EAAB-41B5-8A87-123C57EF7883.png`
- Purpose:
  - prove current shell baseline on `My Mac`
  - show that current-head UI evidence is still about Proposal 004/005 shell states, not Proposal 006 screens

## Attempted But Missing

- No Provider Settings screen could be reached because none exists in current app shell.
- No first-run wizard could be reached because none exists in current app shell.
- No diagnostics/preflight report screen could be reached because none exists in current app shell.
- No pilot readiness screen could be reached because none exists in current app shell.
- No support bundle export UI could be reached because none exists in current app shell.

## Evidence Gate Assessment

- repo/code inspection: `met`
- build/run attempt logged: `met`
- platform-appropriate UI screenshots/attachments: `met`, but only for current shell baseline
- completed evidence pack with IDs: `met`

Why the review is still `Partial` instead of `Complete`:
- the evidence gate is satisfied for a draft-readiness review and current-head baseline
- the actual Proposal 006 product surfaces do not exist yet, so there is no live implemented-flow evidence for them
