# Proposal 006: Provider Expansion — Multi-Provider Routing, Settings, Diagnostics, and MVP Pilot Readiness Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` |
| Repository Root | `.` |
| Git SHA | `c62515f` |
| Reviewed At | `2026-03-24T09:01:22+0200` |
| Proposal Source MD5 | `c5238fbe299bfb7bd21c33fe558fecde` |
| Proposal Source MTime | `2026-03-24 08:47:38 +0200` |
| Review Mode | `full-review` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Yellow` |
| Confidence | `Medium` |
| Evidence Completeness | `Partial` |

## Verdict

Proposal 006 is materially healthier than the previous review file suggested. The old `ConfiguredProvider.status` finding is closed in the current draft: `ConfiguredProvider` is now durable configuration only, `ProviderHealthSnapshot` is modeled separately, and the text explicitly says health is derived runtime state rather than persisted truth.

Two narrower draft issues remain. First, the settings transfer contract says import validates file version, but `ExportableSettingsPackage` still has no schema/version field to validate. Second, the proposal documents snake_case configuration-source labels in diagnostics and preflight while the `ConfigurationSource: String, Codable` enum still uses camelCase raw values, leaving the on-disk/on-wire contract internally inconsistent.

This still lands as an `Evidence Gap Review` because Proposal 006 product surfaces do not exist in the running app yet. Fresh repo evidence is now better than the last round: `xcodebuild build` is green and the current macOS UI-baseline slice is green, but the app still has no `ProviderSettingsView`, `FirstRunSetupWizard`, `PreflightReportView`, `PilotReadinessView`, `ProviderRegistry`, `AppConfigurationStore`, `ProviderSettingsStore`, or `SettingsTransferService` implementation to review as a live flow.

## Findings

### ARCH-006-007 — Settings transfer version validation has no schema field

- Severity: `High`
- Confidence: `0.98`
- Evidence: `E-P006-001`

Section 8.5 says settings import validates file version, but `ExportableSettingsPackage` contains no schema or payload version field at all. As written, the proposal requires a compatibility check that its own file format cannot perform. For a migration/onboarding feature, that is a real contract gap rather than an implementation detail.

Required fix:
- Add an explicit transfer schema version field to `ExportableSettingsPackage`.
- State how import handles newer, older, or unsupported versions.
- Keep `appVersion` as product metadata rather than the compatibility key unless the draft intentionally wants to couple file compatibility to app build version.

Acceptance check:
- A reader can point to one concrete field that import uses for version compatibility decisions.

### ARCH-006-008 — `ConfigurationSource` wire values conflict with the documented source labels

- Severity: `Medium`
- Confidence: `0.96`
- Evidence: `E-P006-001`

Section 5.5 says diagnostics and preflight must surface configuration source as `persisted_settings`, `seeded_from_env`, and `development_env_override`, but the actual `ConfigurationSource: String, Codable` enum example uses camelCase raw values (`persistedSettings`, `seededFromEnv`, `developmentEnvOverride`). Because this is a raw-value `Codable` enum, that inconsistency leaks directly into export/import, diagnostics serialization, and any persisted settings package using the type.

Required fix:
- Pick one canonical set of serialized values and use it consistently in the enum and the descriptive sections.
- If the display strings differ from serialized values, say that explicitly and show the mapping.

Acceptance check:
- The proposal defines one unambiguous serialized representation for configuration source values.

## Evidence Summary

- `E-P006-001`: Current reread of `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- `E-P006-002`: Current env-driven bootstrap/runtime path in `Chainworks Forge/Chainworks_ForgeApp.swift`
- `E-P006-003`: Current workspace contract in `Chainworks Forge/Engine/RunPlanCompiler.swift`
- `E-P006-004`: Current app shell and code-search baseline showing Proposal 005 operator surfaces exist, but no Proposal 006 stores/views/services exist in source
- `E-P006-005`: Fresh `xcodebuild build` on current HEAD
  - result: `passed`
  - derived data: `/tmp/codex-dd-proposal006-r3-build`
  - note: build is green, but Swift 6 actor-isolation warnings remain in parser/validator/test code
- `E-P006-006`: Fresh macOS UI-baseline rerun on current HEAD
  - command targeted 4 current-shell UI tests
  - result: `4/4 passed`
  - xcresult: `/tmp/codex-dd-proposal006-r3-ui2/Logs/Test/Test-Chainworks Forge-2026.03.24_08-59-41-+0200.xcresult`
  - observed attachments include:
    - `REQ011_Approvals`
    - `P004_NonHappy_MissingRuntime`
    - `REQ011_RunProgress_Entry`
    - `REQ011_RunProgress_Overview`
    - `REQ011_RunProgress_Sections`
    - `REQ011_Sheet`

## Missing Evidence

- No Proposal 006-specific UI surfaces exist in the running app:
  - no provider settings screen
  - no first-run setup wizard
  - no preflight report screen
  - no pilot readiness screen
- No Proposal 006-specific stores/services exist in current source:
  - no `ProviderRegistry`
  - no `AppConfigurationStore`
  - no `ProviderSettingsStore`
  - no `SettingsTransferService`
  - no `KeychainSecretStore`
  - no `ProviderDiagnosticService`

## What Can Still Be Said With Partial Confidence

- The prior persisted-health finding is closed in the current draft.
- Proposal 006 is now much closer to handoff-ready as a document.
- The remaining live proposal issues are narrower and architectural:
  - settings transfer versioning
  - configuration-source serialization consistency
- Current HEAD is healthy enough to trust the baseline evidence pack:
  - build is green
  - baseline macOS UI slice is green

## What Is Required To Finish The Full Review

- Fix the two remaining draft inconsistencies above.
- Implement at least the first Proposal 006 product surfaces and stores:
  - `ProviderSettingsView`
  - `FirstRunSetupWizard`
  - `PreflightReportView`
  - `PilotReadinessView`
  - corresponding settings/registry services
- Re-run the review with live Proposal 006 flows rather than only the current operator-shell baseline.
