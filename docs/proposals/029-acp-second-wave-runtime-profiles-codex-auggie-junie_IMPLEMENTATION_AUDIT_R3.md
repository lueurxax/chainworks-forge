# Proposal 029: Second-Wave ACP Runtime Profiles Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md` |
| Repository Root | `.` |
| Git SHA | `ab1d8df` |
| Working Tree | dirty |
| Audited At | `2026-04-09T16:48:17+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 029 is stronger than in `R2`: the focused same-tree `proposal-029` gate is now green, second-wave provider families/adapters/settings are in place, and rollout gating via `isEnabled` is materially implemented. But the proposal is still not implemented on the current tree because the amended contract now explicitly owns real executable second-wave transports plus one successful proof path for each in-scope family, and the current tree still ships stub `CodexACPTransport`, `AuggieCLIACPTransport`, and `JunieCLIACPTransport`. MCP registry ownership is also only partially transport-neutral, so the Codex ACP rich-MCP branch is not yet fail-closed on the promised adapter-aware registry/readiness owner.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | `REQ-011` and `REQ-013` remain missing | High |
| Architecture | At Risk | registry ownership is still concretely Goose-first | High |
| Product | At Risk | second-wave profiles can be declared and routed, but not executed | High |
| UI | Acceptable | readiness copy is more generic than the actual registry owner chain | Medium |
| UX | At Risk | family-specific MCP failure semantics are only partially explicit to operators | Medium |
| Readiness | Not Ready | proposal-owned execution proof for Codex/Auggie/Junie is absent | High |

## Proposal Contract

### Scope

- Expand the provider platform with second-wave ACP families: `codexACP`, `auggie`, and `junie`.
- Keep the already-landed catalog/runtime cutovers for `codex_writer_high`, Gemini reviewers, and second-wave orchestrator ACP profiles.
- Make unknown adapter families fail closed instead of silently downgrading to Goose.
- Finish the MCP registry migration so runtime reconciliation is adapter-family-aware rather than Goose-specific.
- Enforce `RuntimeProfile.requires` through `ProviderCapabilities`.
- Complete the remaining execution work so Codex ACP, Auggie CLI ACP, and Junie CLI ACP are real runnable transports, not catalog scaffolding.

### Locked Decisions

- `P029` is not “catalog-only”; it owns provider-platform and runtime execution work.
- Unknown `adapterFamily` values must never silently fall back to Goose.
- `RuntimeProfile.requires` extends `ProviderCapabilities`; it does not create a second authority.
- Codex ACP keeps the `codex` MCP namespace and rich lane mappings.
- Auggie and Junie are zero-MCP-only by design in `P029`.
- `ConfiguredProvider.isEnabled` is the single rollout gate.
- The phase list is sequencing inside one proposal, not deferred future work outside `P029`.

### Primary User Flows

1. The operator sees second-wave providers/configuration in Settings and rollout gating behaves predictably.
2. A run resolves backend profiles onto the correct runtime profile / adapter family without silent Goose fallback.
3. Preflight blocks unsupported adapter families, disabled providers, and unsupported MCP/capability combinations before run start.
4. A run actually executes on Codex ACP, Auggie CLI ACP, or Junie CLI ACP and preserves truthful binding/runtime data into snapshots and reports.

### UI Commitments

- Provider/readiness surfaces must expose configured-but-disabled providers as a rollout gate.
- Preflight must distinguish “provider not enabled” from “capability mismatch”.
- MCP/readiness surfaces must reflect the actual runtime namespace and reconciliation owner.

### UX Commitments

- Operators should get actionable preflight failures instead of silent runtime downgrade.
- Codex/Auggie/Junie runtime behavior should be explicit and deterministic.
- Zero-MCP-only families should fail before execution when MCP-dependent agents target them.

### Acceptance Criteria

The proposal requires:

1. second-wave provider families, adapters, capabilities, and health probes;
2. fail-closed transport selection with owner-chain handling and preflight registration;
3. transport-neutral runtime registry ownership with explicit ACP runtime namespaces;
4. `RuntimeProfile.requires` enforced through `ProviderCapabilities`;
5. every `requires` token mapped to a locked capability field/consumer;
6. Goose default path preserved;
7. run snapshots and reports preserve truth across provider families;
8. rollout enablement stays owned by `ConfiguredProvider.isEnabled`;
9. focused same-tree `proposal-029` gate passes;
10. canonical catalog preserves the landed rollout decisions;
11. runs routed to `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp` no longer fail with stub transport errors;
12. MCP behavior is deterministic per second-wave family;
13. same-tree verification includes one successful execution proof path for each in-scope second-wave family.

### Test / Evidence Requirements

- Same-tree focused `proposal-029` gate.
- Code/test evidence for second-wave provider platform expansion, transport routing, MCP policy behavior, rollout gating, and truthful runtime/report persistence.
- Successful execution proof for each in-scope second-wave family.

### Explicit Exclusions

- No hard cutover away from Goose.
- No operator-grade claim for second-wave providers beyond the evidence.
- No cross-provider MCP parity requirement beyond the family-specific contract locked in `P029`.

## Proposal Fidelity / Divergence

### Matches

- Second-wave `ProviderFamily` cases, capability defaults, and seeded `ConfiguredProvider` entries exist.
- `ProviderAdapterFactory` now includes provider adapters for Codex ACP, Auggie, and Junie.
- Unknown `adapterFamily` values now fail closed in the runtime transport factory.
- Preflight enforces `RuntimeProfile.requires` through `ProviderCapabilities`.
- Disabled-provider rollout semantics are now wired through resolver, preflight, seeded defaults, preferred-provider selection, and preferred-provider repair.
- The canonical catalog preserves `codex_writer_high -> codex_acp`, `gemini_review_pro` for proposal UI/UX reviewers, second-wave ACP orchestrator profiles, and Codex MCP mappings.
- The focused same-tree `proposal-029` gate now passes.
- Runtime namespaces exist for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`.

### Divergences

- All three second-wave transports are still explicit stubs that throw `"is not yet implemented"`.
- MCP registry ownership is still concretely Goose-first in bridge, executor, preflight, and snapshot-building call sites.
- Codex ACP rich-MCP readiness is not yet blocked on the proposal-promised adapter-aware registry provider.
- The proposal-required successful execution proof per in-scope second-wave family is still absent.

### Ambiguities / Evidence Gaps

- No live external Codex/Auggie/Junie subprocess environment was used in this audit; all evidence is same-tree code/tests/gate evidence.
- Because second-wave transports are still stubs, cross-family run-report truth is only partially provable from code/tests rather than live second-wave execution.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 3 |
| Missing | 2 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Second-wave provider-platform expansion exists
- Proposal Source: `§4.1`, `§5.1`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-36`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:118-185`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-287`
  - `Chainworks Forge/Providers/ProviderAdapter.swift:23-33`
  - `Chainworks Forge/Providers/CodexACPProviderAdapter.swift:3-41`
  - `Chainworks Forge/Providers/AuggieProviderAdapter.swift:3-41`
  - `Chainworks Forge/Providers/JunieProviderAdapter.swift:3-41`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:129-210`
- Gap / Note: The provider families, adapter factory registrations, seeded health/readiness adapters, capability defaults, and seeded settings entries are present on the current tree.

### REQ-002 Unknown adapter families fail closed and the owner chain surfaces the error before silent downgrade
- Proposal Source: `§4.2`, `§5.2`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1079-1113`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:52-55`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:147-155`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:279-300`
  - `Chainworks Forge/Engine/PreflightService.swift:205-224`
  - `Chainworks ForgeTests/Proposal029Tests.swift:11-84`
- Gap / Note: The factory throws `unknownAdapterFamily`, executor surfaces session-creation failure instead of downgrading, and preflight blocks unregistered adapter families before run start.

### REQ-003 MCP registry ownership is transport-neutral and resolved through the correct adapter-family owner
- Proposal Source: `§4.3`, `§5.3`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:114-121`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49-66`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:234-249`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:300-323`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-484`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks Forge/Views/IdeaListView.swift:2302-2309`
- Gap / Note: `RuntimeExtensionRegistrySnapshot` and adapter-family runtime namespaces exist, but bridge/executor/preflight/snapshot call sites still instantiate `GooseExtensionRegistryReader()` directly, and installed-extension validation only special-cases `runtimeNamespace == "goose"`. The proposal-promised adapter-aware registry owner is therefore incomplete for Codex ACP.

### REQ-004 Preflight validates `RuntimeProfile.requires` through `ProviderCapabilities`
- Proposal Source: `§4.4`, `§5.4`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:289-303`
  - `Chainworks Forge/Engine/PreflightService.swift:632-665`
  - `Chainworks ForgeTests/Proposal029Tests.swift:122-133`
- Gap / Note: The current preflight path resolves runtime-profile tokens through the configured provider capability owner rather than maintaining a parallel capability authority.

### REQ-005 Every `requires` token maps to a locked capability field and consumer
- Proposal Source: `§4.4`, `§5.5`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-303`
  - `examples/agents/agents.yaml:535-576`
  - `Chainworks Forge/Engine/PreflightService.swift:632-665`
- Gap / Note: The in-scope `requires` vocabulary is the proposal’s narrowed token set, and each token now maps onto a concrete `ProviderCapabilities` field with an enforcement consumer in preflight.

### REQ-006 Goose default path remains operational
- Proposal Source: `§3`, `§4.2`, `§5.6`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1080-1086`
  - `Chainworks ForgeTests/Proposal026Tests.swift:248-320`
- Gap / Note: Goose remains the explicit fallback path only when the binding’s adapter family is `goose` or absent; the same-tree gate still exercises the first-wave ACP path without regressing the legacy Goose default resolution contract.

### REQ-007 Run snapshots and execution reports preserve truthful provider/runtime identity across provider families
- Proposal Source: `§5.7`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Models/Run.swift:43-53`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:883-892`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1894-1976`
  - `Chainworks ForgeTests/Proposal026Tests.swift:480-485`
  - `Chainworks ForgeTests/Proposal026Tests.swift:596-601`
- Gap / Note: The persistence/report path is in place and proven for first-wave ACP runs, but second-wave execution cannot yet be proven through this path because Codex/Auggie/Junie transports still do not execute successfully.

### REQ-008 Rollout enablement uses `ConfiguredProvider.isEnabled` as the single owner
- Proposal Source: `§4.8`, `§5.8`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-36`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:56-63`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:158-181`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:96-150`
  - `Chainworks Forge/Engine/PreflightService.swift:148-157`
  - `Chainworks ForgeTests/Proposal029Tests.swift:137-188`
- Gap / Note: The old preferred-provider repair gap is closed on the current tree: repair now filters to enabled same-family providers, and preflight distinguishes rollout gating from capability mismatch.

### REQ-009 Focused same-tree `proposal-029` gate passes
- Proposal Source: `§5.9`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `scripts/test-gate.sh:128-132`
  - `scripts/test-gate.sh:1370-1379`
  - Command: `bash 'scripts/test-gate.sh' proposal-029`
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-029-20260409-164538.xcresult`
  - Outcome: `Test run with 59 tests in 3 suites passed after 25.373 seconds. ** TEST SUCCEEDED **`
- Gap / Note: This closes the earlier red-gate blocker from `R2`, but it does not by itself satisfy the now-expanded execution-proof contract in `REQ-013`.

### REQ-010 Canonical catalog preserves the landed rollout decisions
- Proposal Source: `§3.1`, `§5.10`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:113-145`
  - `examples/agents/agents.yaml:595-602`
  - `examples/agents/agents.yaml:743-766`
  - `examples/agents/agents.yaml:1183-1213`
- Gap / Note: The current catalog preserves Codex rich MCP mappings, `codex_writer_high -> codex_acp`, Gemini `proposal_reviewer_ux` / `proposal_reviewer_ui -> gemini_review_pro`, and the `structured_output: preferred` second-wave ACP orchestrator profiles.

### REQ-011 Runs routed to `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp` no longer fail with stub transport errors
- Proposal Source: `§3.2`, `§5.11`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:8-41`
- Gap / Note: All three transports still throw `"... is not yet implemented"` in session creation, streaming, and close paths. This is a direct miss against the proposal’s executable-runtime contract.

### REQ-012 MCP behavior is deterministic per second-wave family
- Proposal Source: `§4.3.1`, `§5.12`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `examples/agents/agents.yaml:113-145`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49-66`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:234-249`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:300-323`
  - `Chainworks Forge/Engine/PreflightService.swift:677-735`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1844-1892`
- Gap / Note: Auggie and Junie are effectively zero-MCP-only because they have no runtime mappings and therefore fail MCP resolution deterministically. Codex ACP, however, still lacks the proposal-promised adapter-aware registry provider/readiness gate: the current implementation only validates installed extensions against a live registry for the `goose` namespace, not for `codex`.

### REQ-013 Same-tree verification includes one successful execution proof path for each in-scope second-wave family
- Proposal Source: `§3.2`, `§5.13`
- Status: Missing
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal026Tests.swift:353-405`
  - `Chainworks ForgeTests/Proposal029Tests.swift:11-200`
  - `scripts/test-gate.sh:128-132`
  - Command: `bash 'scripts/test-gate.sh' proposal-029`
- Gap / Note: The current gate executes first-wave ACP proof tests for Claude/Gemini and structural tests for `P029`, but it does not run a successful Codex/Auggie/Junie execution proof. Because the second-wave transports are still stubs, the proposal’s per-family proof requirement cannot yet be satisfied.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Registry migration remains half-complete and still concretely Goose-owned
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`, `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:114-121`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:300-323`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-484`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks Forge/Views/IdeaListView.swift:2302-2309`
- Why It Matters: The proposal locked adapter-family-aware registry ownership as part of the second-wave rollout. The current tree only renamed the snapshot type; actual provider selection still constructs `GooseExtensionRegistryReader()` directly in multiple owners, and installed-extension reconciliation remains Goose-specific. That leaves Codex ACP without the promised registry/readiness authority.
- Recommended Action: Introduce a real adapter-family registry owner chain and replace the direct Goose reader construction in bridge, executor, preflight, and snapshot-building paths.

## Product Review

**Summary:** At Risk

### PROD-001 Second-wave runtime profiles are still routable but not executable
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-011`, `REQ-013`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:8-41`
  - `bash 'scripts/test-gate.sh' proposal-029`
- Why It Matters: The amended proposal no longer treats second-wave ACP as structural scaffolding only. It explicitly owns executable Codex/Auggie/Junie transports and successful proof per family. Current catalog/runtime resolution can route work toward these transports, but the transports still fail immediately with stub errors, so the core user value is still unavailable.
- Recommended Action: Implement real session creation, streaming, and close semantics for all three transports and add executed proof tests for each family to the `proposal-029` gate.

### PROD-002 Codex ACP rich-MCP readiness remains weaker than the proposal contract
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`, `REQ-012`
- Evidence Type: code, tests-found
- Evidence:
  - `examples/agents/agents.yaml:113-145`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:315-323`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1844-1892`
- Why It Matters: The proposal’s Codex ACP branch promises mapped-lane support plus fail-closed blocking when the Codex registry/readiness source cannot satisfy those lanes. Current code proves the catalog mappings exist and can pass preflight, but it does not yet enforce Codex-specific registry presence/readiness. That leaves a product contract gap even before transport execution is implemented.
- Recommended Action: Add an adapter-aware Codex registry provider and make Codex MCP-dependent agents fail preflight when that provider is unavailable or cannot validate the mapped lane.

## UI Review

**Summary:** Acceptable

### UI-001 Preflight/readiness copy is more generic than the actual MCP owner chain
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-003`, `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:706-711`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-484`
- Why It Matters: The operator-facing preflight surface now says `Runtime Extension Registry`, which is directionally correct, but the underlying implementation is still concretely Goose-owned. That can overstate how transport-neutral the runtime is today, especially for Codex ACP.
- Recommended Action: Surface family-specific registry owner/readiness state in the UI or fail closed earlier so the generic label never over-promises the current implementation.

## UX Review

**Summary:** At Risk

### UX-001 Family-specific MCP failure semantics are only partially explicit to operators
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-012`
- Evidence Type: code, inference
- Evidence:
  - `examples/agents/agents.yaml:113-145`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:312-323`
  - `Chainworks Forge/Engine/PreflightService.swift:713-735`
- Why It Matters: The amended proposal intentionally narrowed family semantics: Codex gets mapped lanes; Auggie and Junie are zero-MCP-only. Current behavior is deterministic in code, but it is still surfaced mostly as generic missing mapping / generic runtime registry feedback rather than as explicit family policy. That weakens operator clarity and makes troubleshooting less direct than the proposal intends.
- Recommended Action: Add family-aware preflight messaging for zero-MCP-only families and for the missing Codex registry-provider case.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Proposal-owned proof for each second-wave family is still absent
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-011`, `REQ-013`
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:8-41`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:8-41`
  - `Chainworks ForgeTests/Proposal026Tests.swift:353-405`
  - `Chainworks ForgeTests/Proposal029Tests.swift:11-200`
  - `bash 'scripts/test-gate.sh' proposal-029`
- Why It Matters: `P029` now explicitly says it is incomplete until all listed phases are complete and requires one successful execution proof path per in-scope second-wave family. The current tree has a passing focused gate, but that gate still proves first-wave execution plus second-wave structure, not successful second-wave transport execution.
- Recommended Action: Add Codex/Auggie/Junie proof tests that actually execute the second-wave transport path and keep them in the focused `proposal-029` gate.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `bash 'scripts/test-gate.sh' proposal-029` built and tested successfully on macOS |
| Core user flow runtime-validated | Partial | first-wave ACP execution proof runs under the focused gate; second-wave execution proof is still missing |
| Empty/loading/error states covered | Partial | rollout/preflight error paths exist in code, but the proposal is runtime-platform-first rather than UI-state-first |
| Accessibility risk acceptable | Not Checked | not a primary audit axis for this runtime slice |
| Localization risk acceptable | Not Checked | not reviewed in this pass |
| Critical tests executed | Pass | `proposal-029` focused gate passed on the same tree/HEAD |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | no full `scripts/test-gate.sh full` run in this audit |
| Privacy/permissions/entitlements reviewed | Not Checked | not a primary proposal claim in this pass |

## Verification Log

- `git status --short`
- `date '+%Y-%m-%dT%H:%M:%S%z'`
- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md'`
- `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md' 'docs/proposals' 'docs/reviews' 'docs/reference'`
- `rg -n "not yet implemented|mcp_server_registry:|runtime_profiles:|codex_acp|auggie_cli_acp|junie_cli_acp|GooseExtensionRegistryReader|Runtime Extension Registry|supportsMCPReconciliation|removeProvider\\(|preferredProvider\\(|providerNotEnabled\\(|effectiveRuntimeNamespace|proposal-029|Claude Agent ACP-backed canonical proposal loop|Gemini CLI ACP-backed canonical proposal loop|implementation path reaches manual release gate" 'Chainworks Forge' 'Chainworks ForgeTests' 'examples/agents/agents.yaml' 'scripts/test-gate.sh'`
- `bash 'scripts/test-gate.sh' proposal-029`

## Recommended Next Actions

1. Implement `CodexACPTransport`, `AuggieCLIACPTransport`, and `JunieCLIACPTransport` end-to-end and replace the current stub errors in create/stream/close.
2. Finish adapter-family-aware runtime registry ownership so Codex ACP fails closed when its registry/readiness provider is unavailable, and remove the remaining direct `GooseExtensionRegistryReader()` construction from bridge/executor/preflight/snapshot paths.
3. Add explicit `AC-12` / `AC-13` proof tests: Codex rich-MCP blocking on missing registry provider, Auggie/Junie zero-MCP-only preflight behavior, and one successful execution proof per second-wave family.
4. Re-run `bash 'scripts/test-gate.sh' proposal-029` after the above and only then consider broader regression sign-off.
