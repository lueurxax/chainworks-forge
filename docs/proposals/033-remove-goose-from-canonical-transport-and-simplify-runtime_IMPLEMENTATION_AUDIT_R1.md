# Proposal 033: Complete Goose Removal and ACP-Only Runtime Architecture Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` |
| Repository Root | `.` |
| Git SHA | `d9aa3b2e82ac5c455da027f9cf045dbe93173273` |
| Working Tree | `clean` |
| Audited At | `2026-04-10T21:32:52+03:00` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P033` is only partially landed. The repository already contains a meaningful ACP-first provider migration (`ProviderFamily`/`ProviderTransport` cleanup, ACP-only provider adapters, `runtimeSessionID` persistence rename), but the canonical runtime, MCP, configuration, fixture/test, operator UI, docs, blocked-run recovery, and proof-lane layers are still Goose-shaped. Because several in-scope `REQ-*` items are still outright missing, conformance is `Not Implemented`, and readiness is `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Core runtime still depends on Goose compatibility paths | High |
| Architecture | Weak | Dual-path transport/MCP/config ownership remains canonical | High |
| Product | At Risk | Operator still has to think in Goose terms for live execution | High |
| UI | At Risk | Multiple operator-facing surfaces still show Goose-specific wording | Medium |
| UX | At Risk | Migration/recovery story for old Goose-bound runs is not implemented | High |
| Readiness | Not Ready | No `proposal-033` proof lane, and `P030` prerequisite is still red | High |

## Proposal Contract

### Scope

- Remove all Goose runtime code.
- Make every runtime path ACP-only.
- Refactor transport, MCP, session, executor, configuration, provider platform, fixture/test, UI, docs, and recovery surfaces accordingly.

### Locked Decisions

- `P033` implementation cannot start until `P030` is `Implemented / Ready`.
- Brand/design references to geese are out of scope; runtime/config/operator references are not.
- Historical Goose-bound runs are blocked, not converted.
- `gooseSessionID` is migrated to `runtimeSessionID` with `@Attribute(originalName:)`.
- Provider settings migration must happen on raw JSON before `JSONDecoder`.

### Primary User Flows

1. Operator starts a live run on an ACP-only runtime without Goose setup or Goose-first messaging.
2. Existing local and imported provider settings migrate safely from Goose-era values into ACP-only provider state.
3. Historical Goose-bound runs are surfaced as permanently blocked with neutral remediation.
4. Engineers validate the ACP-only architecture through repo-owned `proposal-033` proof gates and updated reference docs.

### UI Commitments

- `ProviderSettingsView`, `FirstRunSetupWizard`, `IdeaListView`, `PilotReadinessView`, and `RunsHomeView` should stop using Goose-specific operator wording.
- Goose assistant/remediation surfaces should be deleted.
- Operator-facing strings should not mention “Goose”.

### UX Commitments

- Recovery and blocked-run messages should be transport-neutral.
- Trust labels should read as legacy/runtime truth, not “Goose server”.
- Live runtime setup should guide ACP, not Goose env vars or Goose server configuration.

### Acceptance Criteria

The proposal explicitly requires, among other things:

1. zero Goose adapter files in runtime code,
2. zero Goose-backed provider families/transports,
3. zero Goose config/env vars in code,
4. zero Goose namespaces in MCP policy,
5. zero Goose wording in operator-facing UI,
6. durable settings migration for old `provider-settings.json` values,
7. legacy `gooseSessionID` persistence rename,
8. `ResumeManager` blocking for Goose-bound runs,
9. a repo-owned `proposal-033` test gate with `P030` as prerequisite.

### Test / Evidence Requirements

- `Proposal033Tests` must exist and prove ACP-only transport/MCP/settings migration/history handling.
- `test-gate.sh` must expose `proposal-033|p033`.
- Any successful implementation verdict would require same-tree full regression. This audit does not claim a successful verdict.

### Explicit Exclusions

- Completing `P030`.
- Converting historical Goose runs into ACP runs.
- Removing brand/design metaphors that use geese visually.

## Proposal Fidelity / Divergence

### Matches

- `AgentExecution` already stores `runtimeSessionID` with `@Attribute(originalName: "gooseSessionID")`.
- `ProviderFamily` and `ProviderTransport` are already ACP-only in the durable model.
- `ProviderAdapterFactory` is already ACP-only.

### Divergences

- Core runtime transport still uses Goose compatibility owners and defaults.
- MCP policy still relies on `GooseExtensionRegistryReader`, Goose namespace handling, and Goose config paths.
- Config/bootstrap still expose `gooseServer*` fields and `CHAINWORKS_GOOSE_*` env vars.
- Durable settings migration hooks promised in `3.6a` do not exist.
- Fixture/test layer is still Goose-shaped.
- Operator UI and reference docs still speak in Goose-first language.
- `ResumeManager` does not block Goose-bound runs.
- `proposal-033` gate and `Proposal033Tests` do not exist.

### Ambiguities / Evidence Gaps

- No blocking proposal-evidence gap remains. Local code/docs/test-gate evidence is sufficient.
- One targeted `xcodebuild test` run was started for `ProviderPlatformTests` and `RuntimeSessionBridgeTests`; Swift Testing suites began execution, but the terminal pass/fail completion was not captured cleanly, so it is not used as success proof.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 2 |
| Missing | 9 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal-owned proof lane exists with `proposal-033` gate and `Proposal033Tests`
- Proposal Source: `§2 Hard Prerequisite Gate and Proof Lane`
- Status: `Missing`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `scripts/test-gate.sh:128`
  - `scripts/test-gate.sh:1150`
  - `scripts/test-gate.sh:1370`
  - `docs/reference/test-gates.md:1`
  - `rg -n "Proposal033Tests" 'Chainworks ForgeTests' 'Chainworks Forge'` returned no matches
- Gap / Note: The repo still exposes `proposal-029` and `proposal-032`, but no `PROPOSAL_033_TESTS`, no `proposal-033|p033`, and no `Proposal033Tests`.

### REQ-002 Transport layer becomes ACP-only with Goose transport code removed
- Proposal Source: `§3.1 Transport Layer`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:5`
  - `Chainworks Forge/Engine/ExecutionService.swift:855`
  - `Chainworks Forge/Engine/ExecutionService.swift:872`
  - `Chainworks Forge/Engine/ExecutionService.swift:1097`
- Gap / Note: `ExecutionService` still defines `GooseTransportAPI`, still constructs `gooseTransport`, still resolves `FixtureGooseTransport`, and `DefaultRuntimeTransportFactory` still defaults to family `"goose"`.

### REQ-003 MCP layer removes Goose namespace, registry reader, and Goose config-path ownership
- Proposal Source: `§3.2 MCP Layer`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:121`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:148`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:315`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:392`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:29`
- Gap / Note: Goose registry ownership is still canonical. `GooseExtensionRegistryReader` remains live, uses `CHAINWORKS_GOOSE_CONFIG_PATH`, and `MCPPolicyResolver` still has special-case `"goose"` validation.

### REQ-004 Session/executor layer becomes Goose-free and transport-neutral
- Proposal Source: `§3.3 Session Bridge Layer`, `§3.4 Executor Layer`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:24`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:66`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:163`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:46`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:489`
  - `scripts/test-gate.sh:52`
  - `scripts/test-gate.sh:85`
- Gap / Note: The file/type names are already runtime-neutral, but behavior is not. `RuntimeSessionBridge` still defaults to `GooseExtensionRegistryReader`; `RuntimeAgentExecutor` still exposes `gooseTransportForCancellation`, dispatches Goose registry as default, and still falls back to `"goose"` transport values.

### REQ-005 Persistent model renames `gooseSessionID` to `runtimeSessionID` with backward-compatible migration
- Proposal Source: `§3.4a Persistent Model Migration`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/AgentExecution.swift:16`
  - `Chainworks Forge/Engine/SupportBundleExporter.swift:146`
- Gap / Note: This is landed exactly as proposed: the stored field is `runtimeSessionID` with `@Attribute(originalName: "gooseSessionID")`, and support-bundle export uses the new key.

### REQ-006 Configuration layer removes Goose server fields and `CHAINWORKS_GOOSE_*` env vars
- Proposal Source: `§3.5 Configuration Layer`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Support/AppConfiguration.swift:9`
  - `Chainworks Forge/Support/AppConfiguration.swift:57`
  - `Chainworks Forge/Support/BootstrapConfigurationResolver.swift:37`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:975`
- Gap / Note: `AppConfiguration` still stores `gooseServer*` fields and `gooseServerBaseURL`; bootstrap still reads `CHAINWORKS_GOOSE_BASE_URL`, `CHAINWORKS_GOOSE_API_KEY`, and `CHAINWORKS_GOOSE_BINARY_PATH`; app fixture bootstrap still keys on `CHAINWORKS_GOOSE_FIXTURE_MODE`.

### REQ-007 Provider model uses ACP-only canonical families/transports/adapters
- Proposal Source: `§3.6 Provider Platform Layer`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:107`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:162`
  - `Chainworks Forge/Providers/ProviderAdapter.swift:18`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:129`
- Gap / Note: The durable provider model is already ACP-only: `ProviderFamily` is `codexACP/claudeACP/geminiACP/auggie/junie`, `ProviderTransport` no longer has `.gooseServer`, adapter factory is ACP-only, and seeded settings are ACP-first.

### REQ-008 Durable settings migration exists for Goose-era local and transfer payloads before decode
- Proposal Source: `§3.6a Durable Settings Migration`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:20`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:81`
  - `Chainworks Forge/Support/SettingsTransferService.swift:78`
  - `rg -n "migrateFromGooseEra\\(|migrateRawProviderSettings\\(|migrateRawTransferPackage\\(" 'Chainworks ForgeTests' 'Chainworks Forge'` returned no matches
- Gap / Note: The proposal’s raw pre-decode migration seam is not implemented. Both local settings load and transfer import still decode typed models first, and none of the promised migration helpers exist.

### REQ-009 Fixture/test layer is ACP-shaped rather than Goose-shaped
- Proposal Source: `§3.7 Fixture / Test Layer`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Engine/FixtureACPTransport.swift:3`
  - `Chainworks Forge/Engine/FixtureACPTransport.swift:6`
  - `Chainworks Forge/Engine/FixtureACPTransport.swift:24`
  - `scripts/test-gate.sh:52`
  - `scripts/test-gate.sh:85`
  - `scripts/test-gate.sh:120`
  - `Chainworks ForgeTests/MVPGoldenRunTests.swift:51`
- Gap / Note: The filename has moved to `FixtureACPTransport.swift`, and `RuntimeSessionBridgeTests.swift` exists, but the class is still `FixtureGooseTransport`, its MCP namespace is still `"goose"`, tests still reference Goose fixtures/names, and the old Goose-specific test lanes remain canonical.

### REQ-010 Operator-facing UI surfaces stop using Goose wording and Goose-first remediation
- Proposal Source: `§3.8 UI Layer`, `§7 Acceptance Criteria #11`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:40`
  - `Chainworks Forge/Views/IdeaListView.swift:1488`
  - `Chainworks Forge/Views/IdeaListView.swift:1652`
  - `Chainworks Forge/Views/IdeaListView.swift:1703`
  - `Chainworks Forge/Views/IdeaListView.swift:1926`
  - `Chainworks Forge/Views/IdeaListView.swift:2803`
  - `Chainworks Forge/Views/RunsHomeView.swift:640`
- Gap / Note: Multiple operator surfaces still tell the user that live mode is Goose-backed, require Goose runtime availability, or point them at Goose env vars.

### REQ-011 Reference/doc fallout is applied repo-wide for ACP-only runtime truth
- Proposal Source: `§3.9 Docs Layer`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `docs/reference/README.md:28`
  - `docs/reference/README.md:40`
  - `docs/reference/current-system-baseline.md:27`
  - `docs/reference/current-system-baseline.md:55`
  - `docs/reference/current-system-baseline.md:59`
  - `docs/reference/goose-server-transport.md:1`
  - `docs/reference/acp-runtime-transport.md:138`
  - `docs/reference/live-provider-execution-slice.md:49`
- Gap / Note: The reference layer still treats Goose compatibility as implemented baseline and still ships a dedicated `goose-server-transport.md` reference.

### REQ-012 Historical Goose-bound runs are blocked permanently with neutral explicit error
- Proposal Source: `§4 Handling Existing Goose-Bound Runs`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:137`
  - `Chainworks Forge/Engine/ResumeManager.swift:178`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:828`
  - `Chainworks Forge/Views/RunsHomeView.swift:640`
- Gap / Note: `ResumeManager.classifyRun()` has no Goose-bound binding check. The current recovery path still classifies by compiler/snapshot/workspace logic only, while runtime truth still persists Goose-era adapter values elsewhere.

### REQ-013 Legacy trust/read-model vocabulary becomes transport-neutral
- Proposal Source: `§6 Trust Model`
- Status: `Missing`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/Run.swift:43`
  - `Chainworks Forge/Views/RunsHomeView.swift:640`
  - `Chainworks Forge/Views/RunsHomeView.swift:641`
- Gap / Note: Persisted trust values still document `server_unverified` / `server_verified`, and the UI still renders them as “Goose server / trust pending” and “Goose server / verified” instead of the proposal’s `Legacy (...)` vocabulary.

## Architecture Review

**Summary:** Weak

### ARCH-001 Core runtime is still Goose-shaped, not ACP-only
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `§3.1`, `§3.2`, `REQ-002`, `REQ-003`, `REQ-004`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:5`
  - `Chainworks Forge/Engine/ExecutionService.swift:855`
  - `Chainworks Forge/Engine/ExecutionService.swift:1097`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:121`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:29`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:47`
- Why It Matters: The proposal’s central architectural promise is simplification to one ACP-native runtime model. The current implementation still keeps Goose as the default/fallback transport, the default MCP registry owner, and the cancellation/session compatibility path. That means the canonical runtime architecture has not actually been simplified yet.
- Recommended Action: Remove Goose transport resolution from `ExecutionService` and `DefaultRuntimeTransportFactory`, delete Goose registry ownership from MCP/session bridge/executor layers, and make missing ACP binding data fail closed without Goose defaults.

### ARCH-002 Durable settings migration is the most dangerous missing implementation seam
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§3.6a`, `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:81`
  - `Chainworks Forge/Support/SettingsTransferService.swift:82`
  - `rg -n "migrateFromGooseEra\\(|migrateRawProviderSettings\\(|migrateRawTransferPackage\\(" 'Chainworks ForgeTests' 'Chainworks Forge'` returned no matches
- Why It Matters: The durable model already moved to ACP-only provider enums. Without the promised raw pre-decode migrators, any persisted Goose-era settings payload remains vulnerable to decode failures or silent loss of the row-level semantics the proposal explicitly promised to preserve.
- Recommended Action: Implement the raw JSON migrators in both `ProviderSettingsStore` and `SettingsTransferService`, add UUID-preservation and placeholder-rewrite tests, and only then remove the last compatibility assumptions.

## Product Review

**Summary:** At Risk

### PROD-001 The primary operator job is still framed as Goose-backed live execution
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `Primary User Flow 1`, `REQ-010`, `REQ-011`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:40`
  - `Chainworks Forge/Views/IdeaListView.swift:1488`
  - `Chainworks Forge/Views/IdeaListView.swift:1652`
  - `Chainworks Forge/Views/IdeaListView.swift:1703`
  - `docs/reference/current-system-baseline.md:27`
- Why It Matters: The proposal is not a purely internal refactor. It changes the operator’s mental model from “Goose-backed live execution” to “ACP-only runtime architecture.” The current product experience still teaches the old model, so the intended user value of the proposal is not delivered yet.
- Recommended Action: Migrate the product copy, setup journey, missing-runtime recovery, and baseline docs as one cohesive operator-facing slice instead of treating them as secondary cleanup.

## UI Review

**Summary:** At Risk

### UI-001 Zero-Goose operator-facing string contract is not met
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `§3.8`, `§7 Acceptance Criteria #11`, `REQ-010`, `REQ-013`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ProviderSettingsView.swift:40`
  - `Chainworks Forge/Views/IdeaListView.swift:1488`
  - `Chainworks Forge/Views/IdeaListView.swift:1926`
  - `Chainworks Forge/Views/IdeaListView.swift:2803`
  - `Chainworks Forge/Views/RunsHomeView.swift:640`
  - `Chainworks Forge/Views/RunsHomeView.swift:641`
- Why It Matters: Even if backend refactoring were further along, the proposal explicitly promises zero Goose wording in operator-facing surfaces. The current UI still makes Goose visible in mode selection, missing-runtime help, session cards, and trust badges.
- Recommended Action: Replace live-mode operator copy, trust badges, and assistant remnants together, then add snapshot/UI coverage keyed to “no Goose operator strings”.

## UX Review

**Summary:** At Risk

### UX-001 Migration and recovery semantics for historical Goose runs are not implemented
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `Primary User Flow 3`, `REQ-012`, `REQ-013`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:137`
  - `Chainworks Forge/Views/RunsHomeView.swift:640`
  - `Chainworks Forge/Models/Run.swift:43`
- Why It Matters: The proposal explicitly chooses a conservative UX: historical Goose runs stay readable but become permanently blocked with neutral remediation. Today that recovery/inspection behavior is not encoded, so a migration would leave operators with mixed semantics instead of one clear path.
- Recommended Action: Add a Goose-bound run classifier in `ResumeManager`, normalize legacy trust strings on read, and cover the blocked-run UX with focused tests before removing the compatibility path.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Proposal-owned proof lane is absent and the external prerequisite is still red
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `§2`, `REQ-001`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `scripts/test-gate.sh:128`
  - `scripts/test-gate.sh:1150`
  - `scripts/test-gate.sh:1370`
  - `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md:1`
- Why It Matters: Even if more implementation landed tomorrow, the proposal’s own delivery contract is still unfulfilled: there is no repo-owned `proposal-033` proof lane, and the prerequisite `P030` audit remains red. That makes sign-off and handoff unsafe.
- Recommended Action: Land `P030` first, then add `PROPOSAL_033_TESTS` plus `proposal-033|p033` to `test-gate.sh` and wire the gate into `docs/reference/test-gates.md`.

### READY-002 Focused verification is incomplete and cannot substitute for the proposal-owned proof
- Severity: `Major`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `§2`, `§7`, `REQ-001`, `REQ-009`
- Evidence Type: `tests-run`, `tests-found`
- Evidence:
  - `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/RuntimeSessionBridgeTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
  - partial console evidence showed Swift Testing suites `RuntimeSessionBridge` and `ProviderPlatform` starting and individual tests passing, but terminal pass/fail completion was not captured cleanly
- Why It Matters: There is some real ACP/provider/session coverage in the repo, but it does not prove `P033` readiness. The proposal requires a dedicated gate, and the partial focused run here cannot replace that.
- Recommended Action: Treat the current targeted run only as exploratory evidence. Do not use it for a success claim; add the dedicated proposal gate once the implementation scope actually exists.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Focused macOS `xcodebuild test` reached real build and test execution for targeted suites, but final terminal completion was not captured cleanly. |
| Core user flow runtime-validated | Not Checked | No end-to-end runtime proof for ACP-only live execution was run in this audit. |
| Empty/loading/error states covered | Partial | Code inspection shows missing-runtime UX still uses Goose-first copy. |
| Accessibility risk acceptable | Not Checked | No runtime or UI automation audit for accessibility was performed. |
| Localization risk acceptable | Partial | Goose-specific hard-coded English copy remains in operator surfaces. |
| Critical tests executed | Partial | Focused `ProviderPlatformTests` / `RuntimeSessionBridgeTests` run was started; no `Proposal033Tests` exists. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not required because the verdict is not successful; no same-tree full regression proof exists. |
| Privacy/permissions/entitlements reviewed | Not Checked | Outside this focused audit slice. |

## Verification Log

- `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md'`
- `rg -n "gooseTransport|GooseServerTransport|GooseServerManager|GooseStreamEventMapper|GooseTransport|resolveGooseTransport|gooseServerManager|liveRuntimeConfiguration|CHAINWORKS_GOOSE_|gooseServer" 'Chainworks Forge' 'scripts' 'docs/reference'`
- `rg -n "runtimeNamespace == \"goose\"|\\bgoose\\b|GooseExtensionRegistryReader|usesGooseExecutionPath|effectiveRuntimeNamespace|mcp_profile|mcp_server_registry" 'Chainworks Forge/Engine' 'Chainworks Forge/DSL' 'Chainworks ForgeTests'`
- `rg -n 'Goose|goose server|goose backend|Goose backend|Goose Server|CHAINWORKS_GOOSE_' 'Chainworks Forge/Views' 'Chainworks Forge/Support' 'Chainworks Forge/Chainworks_ForgeApp.swift'`
- `rg -n 'proposal-033|p033|PROPOSAL_033_TESTS|proposal-029|PROPOSAL_029_TESTS' 'scripts/test-gate.sh' 'docs/reference/test-gates.md'`
- `rg -n "Proposal033Tests|migrateFromGooseEra\\(|migrateRawProviderSettings\\(|migrateRawTransferPackage\\(" 'Chainworks ForgeTests' 'Chainworks Forge'`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/RuntimeSessionBridgeTests' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`

## Recommended Next Actions

1. Finish the missing architectural core before any cosmetic cleanup: remove Goose transport ownership from `ExecutionService`, `DefaultRuntimeTransportFactory`, `MCPPolicyRuntime`, `RuntimeSessionBridge`, and `RuntimeAgentExecutor`.
2. Implement the raw pre-decode settings migration path in both `ProviderSettingsStore` and `SettingsTransferService`, including UUID preservation for migrated rows and Codex re-auth semantics.
3. Land the operator-facing migration slice together: remove Goose strings from `ProviderSettingsView`, `IdeaListView`, `RunsHomeView`, and related setup/recovery surfaces.
4. Implement blocked historical-run handling and legacy trust normalization in `ResumeManager` and read-model surfaces.
5. Add `Proposal033Tests`, `proposal-033|p033`, and the proposal-owned proof contract only after `P030` is green.
