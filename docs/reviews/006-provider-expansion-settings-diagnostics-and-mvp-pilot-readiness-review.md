# Proposal 006: Provider Expansion — Multi-Provider Routing, Settings, Diagnostics, and MVP Pilot Readiness Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md` |
| Repository Root | `.` |
| Git SHA | `59b28ea` |
| Reviewed At | `2026-03-23T23:26:09+0200` |
| Proposal Source MD5 | `faf3a79190cd7661160e7fd5df2d9c64` |
| Proposal Source MTime | `2026-03-23 21:35:02 +0200` |
| Review Mode | `full-review` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Red` |
| Confidence | `High` |
| Evidence Completeness | `Partial` |

## Verdict

Proposal 006 is not handoff-ready in its current draft. The current repo proves only the pre-Proposal-006 baseline: env-driven live runtime wiring, provider/model receipts in the live execution slice, and the existing app shell around Ideas, Approvals, Agent Catalog, and Workflow Inspector. The proposal still leaves core product state undefined at the exact place where it wants to add settings, onboarding, diagnostics, and pilot readiness.

The biggest gap is not cosmetic UI polish. It is missing source-of-truth design. The draft says the provider registry becomes the canonical runtime truth and that a first-run wizard chooses workspace roots and YAML locations, but the only persisted settings model shown is provider-only and there is no design for how those operator choices actually become app-wide runtime inputs after relaunch. In parallel, the acceptance criteria add settings export/import without any implementation path in the architecture.

## Findings

### ARCH-006-001 — Proposal 006 overstates its dependency baseline

- Severity: `High`
- Confidence: `0.98`
- Evidence: `E-P006-001`, `E-P006-002`

The context says Proposals 003 and 004 are assumed to have delivered “real execution and implementation/release slices,” but that delivery/runtime loop is explicitly defined later in Proposal 007, not in Proposal 006’s declared dependency set. This matters because Proposal 006 positions itself as MVP pilot readiness, and its operator expectations become misleading if the repo-backed implementation/release path is still future work.

Required fix:
- Rewrite the baseline statement so Proposal 006 depends only on what 001–005 actually provide.
- If pilot readiness in this draft truly depends on repo-backed implementation/release flows, add Proposal 007 as a prerequisite instead of implying those slices already landed.

Acceptance check:
- The dependency and context sections no longer claim implementation/release delivery slices are already available from 003/004.

### ARCH-006-002 — Wizard-selected YAML/workspace paths have nowhere to persist

- Severity: `High`
- Confidence: `0.99`
- Evidence: `E-P006-001`, `E-P006-003`, `E-P006-004`

The first-run wizard is supposed to choose workspace roots and YAML locations, and preflight is supposed to validate those paths before run start. But the only persisted settings model in section 8.2 stores configured providers plus two provider-related toggles. There is no app configuration model for workflow/catalog paths, workspace roots, or worktree base paths, and the file structure adds no such store. As written, the wizard’s most important choices either evaporate after dismissal or remain split across ad hoc runtime code paths.

Required fix:
- Add a first-class persisted app configuration model for workspace root, artifact/worktree base path, workflow source path, and agent catalog path.
- Name the owning service and file placement explicitly.
- Update file structure and acceptance criteria to reference that source of truth.

Acceptance check:
- The proposal shows where YAML/workspace choices are stored, how they survive relaunch, and which screens/services consume them.

### ARCH-006-003 — The proposal adds settings import/export without any implementation path

- Severity: `High`
- Confidence: `0.98`
- Evidence: `E-P006-001`

The acceptance criteria require that settings export/import excludes secrets cleanly, but the architecture never defines a settings export/import component, file format, UI entry point, or merge behavior. The only export mechanism in scope is the support bundle, which is a diagnostics artifact, not a reusable configuration package. For a proposal centered on onboarding and new-machine readiness, this is a missing core slice, not a detail.

Required fix:
- Either remove settings import/export from Proposal 006 acceptance criteria, or add a concrete design for it.
- If it stays in scope, define the exported schema, import validation rules, secret placeholders, and the UI/action entry points.

Acceptance check:
- The architecture contains an explicit import/export path consistent with the acceptance criteria.

### ARCH-006-004 — The new provider registry conflicts with the current env-based runtime contract unless migration rules are explicit

- Severity: `High`
- Confidence: `0.97`
- Evidence: `E-P006-001`, `E-P006-003`, `E-P006-005`

Section 5 says the Provider Registry becomes the single runtime source of truth for configured providers and health. But the current app still bootstraps live runtime exclusively from environment variables and exposes that state directly in start-run messaging. Proposal 006 does not define precedence, migration, or fallback behavior between legacy env configuration and the new in-app settings/Keychain path. Without that, the product can end up with two conflicting truths: a configured provider in Settings and a different live runtime actually used by execution.

Required fix:
- Add an explicit migration and precedence contract:
  - whether env vars remain supported after Proposal 006,
  - whether they seed first-run settings once,
  - whether env vars override persisted settings for development only,
  - and how diagnostics/preflight explain the active source.
- Update the file structure to include the affected bootstrap/runtime surfaces.

Acceptance check:
- A reader can tell exactly how app launch resolves provider config after Proposal 006 and how operator settings reach the execution layer.

## Evidence Summary

- `E-P006-001`: Proposal reread of `docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
- `E-P006-002`: Roadmap cross-check against `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `E-P006-003`: Current app bootstrap and runtime config code in `Chainworks Forge/Chainworks_ForgeApp.swift`, `Chainworks Forge/Engine/ExecutionService.swift`, and `Chainworks Forge/Views/IdeaListView.swift`
- `E-P006-004`: Current app shell/file structure check showing no Proposal 006 surfaces (`ProviderSettingsView`, `FirstRunSetupWizard`, `PreflightReportView`, `PilotReadinessView`, `SupportBundleExporter`, provider registry layer)
- `E-P006-005`: Fresh runtime baseline
  - `xcodebuild build` passed
  - targeted tests passed:
    - `GooseSessionBridgeTests`
    - `GooseAgentExecutorTests`
    - `Chainworks_ForgeUITests.testApprovalInboxReachable()`
    - `Chainworks_ForgeUITests.testLiveRuntimeUnavailableShowsRecoveryGuidance()`
    - `Chainworks_ForgeUITests.testStartRunSheetUI()`
  - xcresult: `/tmp/codex-dd-proposal006-r1/Logs/Test/Test-Chainworks Forge-2026.03.23_23-23-35-+0200.xcresult`
  - fresh UI attachments exported to `docs/reviews/artifacts/proposal-006-provider-r1-ui`

## Missing Evidence

- There is no implemented Proposal 006 runtime slice in the app today:
  - no Provider Settings screen
  - no first-run wizard
  - no diagnostics/preflight screen
  - no pilot readiness screen
  - no support-bundle export UI
- Because those states do not exist, this review cannot produce a true full triad pass over Proposal 006 UI/UX behavior. It can only review draft readiness plus current-head baseline evidence.

## What Can Still Be Said With Partial Confidence

- The current repo is still operating on the Proposal 004/005 shell baseline for provider concerns.
- Proposal 006 is directionally reasonable, but the state-ownership and onboarding architecture is not yet precise enough for clean implementation.
- The missing details are central enough that implementation would otherwise drift into ad hoc path handling and split configuration truth.
