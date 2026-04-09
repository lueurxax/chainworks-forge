# Proposal 029: ACP Second-Wave Runtime Profiles Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `8d79a35` |
| Working Tree | dirty |
| Audited At | `2026-04-09T11:22:08+03:00` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 029 is not implemented end-to-end on the current tree. The repository now has the second-wave `ProviderFamily` cases, disabled-by-default seeded providers, explicit runtime namespaces for Codex/Auggie/Junie, a fail-closed transport factory, and a focused `proposal-029` gate that passes on the same tree. But the proposal’s harder contract is still open in three direct ways: the MCP registry layer remains Goose-specific instead of transport-neutral, the second-wave provider diagnostics/readiness path is not wired because `ProviderAdapterFactory` still only registers first-wave adapters, and the canonical catalog YAML still ships only `claude_agent_acp` and `gemini_cli_acp`. The result is a slice that can prove narrow routing behavior in tests while still failing the proposal’s authoritative platform, catalog, and operator-readiness promises.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Canonical catalog + registry migration are still absent | High |
| Architecture | At Risk | Runtime registry abstraction still returns Goose-specific snapshot types | High |
| Product | At Risk | Advertised second-wave profiles are not actually shippable through canonical catalog and diagnostics paths | High |
| UI | At Risk | Setup/preflight surfaces cannot present second-wave rollout state coherently because health and enablement are incomplete | Medium |
| UX | At Risk | Disabled or missing second-wave providers degrade into generic "no configured provider" / Goose-centric remediation language | High |
| Readiness | Not Ready | Gate passes, but transports remain stubs and key proposal-owned migrations are still missing | High |

## Proposal Contract

### Scope

- Expand the provider platform for second-wave ACP families: `codexACP`, `auggie`, `junie`.
- Make runtime transport selection fail closed for unknown adapter families.
- Migrate MCP registry ownership from Goose-specific terminology to a transport-neutral runtime registry.
- Enforce `RuntimeProfile.requires` through `ProviderCapabilities` as a single capability authority.
- Add rollout enablement through `ConfiguredProvider.isEnabled` with operator-visible "not enabled" semantics.
- Ship canonical catalog/runtime-profile data for the second-wave ACP bindings.

### Locked Decisions

- This is not a catalog-only slice; it explicitly expands the provider platform.
- Unknown adapter families must never silently fall back to Goose.
- `RuntimeProfile.requires` must map into `ProviderCapabilities`, not a parallel capability authority.
- MCP registry ownership must become transport-neutral.
- `ConfiguredProvider.isEnabled` is the single rollout gate.

### Primary User Flows

1. An operator sees second-wave providers in Settings, can enable them deliberately, and can distinguish disabled vs unavailable vs unhealthy states.
2. A runtime profile resolves to Codex/Auggie/Junie without Goose fallback, and preflight blocks unsupported families or capabilities before run start.
3. MCP policy resolution uses the correct runtime namespace/registry per adapter family rather than Goose-only assumptions.
4. Run snapshots and reports preserve which runtime family/profile actually executed.

### UI Commitments

- Provider/readiness surfaces must expose second-wave families as configured-but-disabled by default.
- Preflight must distinguish rollout gating ("not enabled") from capability mismatch and transport failures.

### UX Commitments

- Operators should get actionable failure reasons before run start.
- Second-wave rollout should be safe-by-default and never silently route to Goose.
- ACP-backed MCP support should not inherit Goose-only remediation language when the runtime namespace is not Goose.

### Acceptance Criteria

The proposal requires:

1. new provider families plus seeded settings, capabilities, adapters, and health probes;
2. fail-closed transport factory behavior for unknown adapter families, with preflight validation before run start;
3. transport-neutral MCP registry ownership and explicit MCP namespace per ACP family;
4. preflight enforcement of `RuntimeProfile.requires` through `ProviderCapabilities`;
5. every `requires` token mapped to a locked capability field/consumer;
6. Goose path remains working;
7. run snapshots and reports preserve truth across provider families;
8. rollout enablement is owned by `ConfiguredProvider.isEnabled`, including distinct "not enabled" handling;
9. a focused `proposal-029` gate passes on the canonical tree.

### Explicit Exclusions

- No fallback back to legacy implicit Goose execution for unknown ACP adapter families.
- No parallel capability authority outside `ProviderCapabilities`.

## Proposal Fidelity / Divergence

### Matches

- Second-wave provider families exist on the provider platform.
- Seeded provider settings include disabled-by-default second-wave providers.
- `ProviderRegistry.preferredProvider(for:)` filters by `isEnabled`.
- Runtime namespace mapping exists for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`.
- `DefaultRuntimeTransportFactory` throws for unknown adapter families.
- Focused same-tree `proposal-029` gate passes.

### Divergences

- MCP registry ownership is still Goose-specific all the way down to snapshot types and registry readers.
- Provider diagnostics/health still register only first-wave adapters.
- Canonical `examples/agents/agents.yaml` still omits second-wave runtime profiles and backend profiles.
- `RuntimeProfile.requires` enforcement exists, but the proposal-promised `supportsMCPReconciliation` capability field/token mapping is missing.
- Disabled second-wave providers do not surface a distinct "not enabled" resolution path in the binding/preflight owner chain.

### Ambiguities / Evidence Gaps

- I did not find same-tree runtime proof that Codex/Auggie/Junie ACP transports can execute a live session; the shipped transport classes are still stubbed.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 2 |
| Missing | 3 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Provider platform includes second-wave families with disabled-by-default seeded providers

- Proposal Source: §4.1, §4.8, §5.1, §5.8
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-13`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:118-181`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:150-198`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
- Gap / Note: `ProviderFamily` now includes `codexACP`, `auggie`, and `junie`; seeded settings create disabled-by-default provider rows for all three; and preferred-provider selection filters by `isEnabled`.

### REQ-002 Second-wave providers have real diagnostics / health-probe ownership

- Proposal Source: §4.1, §5.1
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderAdapter.swift:23-30`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:19-27`
- Gap / Note: `ProviderAdapterFactory.makeAdapters()` still returns only `.codex`, `.claude`, and `.gemini`. Second-wave providers have no registered `ProviderAdapter`, so diagnostics/readiness cannot satisfy the proposal’s provider-health ownership.

### REQ-003 Unknown adapter families fail closed before execution and are preflight-validated before run start

- Proposal Source: §4.2, §5.2
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1071-1106`
  - `Chainworks Forge/Engine/PreflightService.swift:611-645`
  - `Chainworks ForgeTests/Proposal029Tests.swift:9-35`
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-029` -> `** TEST SUCCEEDED **`
- Gap / Note: The transport factory now throws `RuntimeTransportError.unknownAdapterFamily`, and the focused proposal gate passed on the same tree. The preflight implementation is still somewhat manual/list-based, but the fail-closed behavior exists.

### REQ-004 MCP registry ownership is transport-neutral, not Goose-specific

- Proposal Source: §4.3, §5.3
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:37-121`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:190-325`
  - `Chainworks Forge/Engine/RuntimeTransport.swift:243-249`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:146-167`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:204-236`
  - `Chainworks Forge/Engine/PreflightService.swift:648-690`
- Gap / Note: The proposal explicitly required `RuntimeExtensionRegistrySnapshot` / transport-neutral registry ownership. Current code still uses `GooseExtensionDefinition`, `GooseExtensionRegistrySnapshot`, `GooseExtensionRegistryReader`, Goose-only validation messages, and a registry-provider protocol that still returns the Goose snapshot type.

### REQ-005 `RuntimeProfile.requires` is enforced through `ProviderCapabilities` as the single capability authority

- Proposal Source: §4.4, §5.4, §5.5
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-299`
  - `Chainworks Forge/Engine/PreflightService.swift:611-645`
  - `Chainworks ForgeTests/Proposal029Tests.swift:120-132`
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-029` -> `** TEST SUCCEEDED **`
- Gap / Note: The owner chain is real: `ProviderCapabilities.satisfies(...)` is called from preflight, and the focused gate covers the basic token mapping. But the proposal promised a new `supportsMCPReconciliation` field/token mapping and a fully locked vocabulary; that field is not present, so the mapping is incomplete against the proposal contract.

### REQ-006 Goose execution path remains working for legacy / non-second-wave execution

- Proposal Source: §5.6
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1071-1078`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1212-1268`
- Gap / Note: The factory still routes empty/legacy adapter families onto the Goose transport, and the tree contains a dedicated live-workflow test for Goose-backed executor routing. I did not run that focused selector successfully in this audit, but the execution path remains present in current code.

### REQ-007 Canonical catalog YAML ships second-wave runtime profiles and backend profiles

- Proposal Source: §4.6, §5.1
- Status: Missing
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:531-552`
- Gap / Note: Canonical `runtime_profiles:` contains only `claude_agent_acp` and `gemini_cli_acp`. I did not find `codex_acp`, `auggie_cli_acp`, `junie_cli_acp`, or the proposal’s second-wave backend-profile entries in the shipped catalog example.

### REQ-008 Rollout enablement distinguishes "not enabled" from capability mismatch or provider absence

- Proposal Source: §4.8, §5.8
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:96-113`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:131-142`
  - `Chainworks Forge/Engine/PreflightService.swift:611-645`
- Gap / Note: Filtering by `isEnabled` exists, but once a family is disabled the resolver still collapses to `noConfiguredProvider`, and preflight capability checks do not surface a distinct "Provider not enabled" rollout gate before capability evaluation.

### REQ-009 Run snapshots and reports preserve provider/runtime-profile truth across families

- Proposal Source: §5.7
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:39-48`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:806-807`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:168-184`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:595-632`
- Gap / Note: The run model persists frozen provider-binding snapshots and provenance, agent executions record `runtimeProfileID` and adapter-family truth, and report payload/markdown surfaces those fields.

### REQ-010 Focused `proposal-029` gate passes on the canonical tree

- Proposal Source: §5.9
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `scripts/test-gate.sh:128-131`
  - `scripts/test-gate.sh:1143`
  - `scripts/test-gate.sh:1362-1371`
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-029` -> `56 tests in 3 suites`, `TEST SUCCEEDED`, result bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-029-20260409-112620.xcresult`
- Gap / Note: The focused gate is real and green on the same tree. It proves the currently implemented slice, but not the full proposal contract.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Runtime registry abstraction is still Goose-specific under the hood

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-004`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift:243-249`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:37-121`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:190-325`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
- Why It Matters: Proposal 029 explicitly moved MCP ownership from Goose-first naming into transport-neutral runtime ownership. The current abstraction only renamed the outer protocol; the core types, resolver parameters, bridge plumbing, and preflight still depend on Goose snapshots. That leaves second-wave ACP support architecturally half-migrated.
- Recommended Action: Complete the type/owner migration to a real runtime-neutral registry contract and add per-adapter registry-provider ownership instead of passing Goose types through a generic wrapper.

### ARCH-002 Provider diagnostics remain first-wave only

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-002`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderAdapter.swift:23-30`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:19-27`
- Why It Matters: The provider platform is supposed to be authoritative for readiness. Without second-wave adapters in the diagnostic factory, the platform cannot honestly assess health, available models, or readiness for Codex/Auggie/Junie.
- Recommended Action: Register real second-wave `ProviderAdapter` implementations and wire them through the same diagnostic owner chain as the first-wave families.

## Product Review

**Summary:** At Risk

### PROD-001 Canonical shipped catalog does not expose the advertised second-wave slice

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:531-552`
- Why It Matters: Product-wise, a proposal that says the second-wave ACP runtime profiles exist is not fulfilled if the shipped authoritative catalog still exposes only Claude/Gemini ACP profiles. Operators and fixture flows cannot consume what the proposal claims to have added.
- Recommended Action: Update the canonical example catalog with the second-wave runtime profiles and backend profiles, then verify resolution/preflight/reporting against those concrete catalog entries.

### PROD-002 Second-wave transports are still stubbed, so the slice is not operator-usable

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-002`, `REQ-010`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:5-42`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:5-42`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:5-42`
- Why It Matters: Even with routing and proof tests, the second-wave profiles are not practically usable if session lifecycle methods still throw "not yet implemented". That keeps the slice in platform scaffolding territory rather than a real provider expansion.
- Recommended Action: Either narrow the shipped scope to scaffolding-only truth or finish the adapter implementations and prove end-to-end execution for at least one second-wave family.

## UI Review

**Summary:** At Risk

### UI-001 Readiness surfaces cannot yet represent second-wave provider state coherently

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-002`, `REQ-008`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:106-107`
  - `Chainworks Forge/Engine/PreflightService.swift:611-645`
  - `Chainworks Forge/Engine/PreflightService.swift:670-690`
- Why It Matters: Proposal 029 turned rollout state into an operator-facing contract. But the current UI/readiness owner chain has no real second-wave health adapters and no dedicated disabled-provider messaging. That means Settings / Preflight / readiness surfaces cannot present the states the proposal promised.
- Recommended Action: Add explicit disabled-provider preflight/readiness presentation and back it with second-wave health adapters so the UI can distinguish disabled, unavailable, unhealthy, and capability-mismatched states.

## UX Review

**Summary:** At Risk

### UX-001 Disabled-provider remediation still collapses into generic provider-absence language

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:106-107`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:138-141`
  - `Chainworks Forge/Engine/PreflightService.swift:627-644`
- Why It Matters: The proposal explicitly promised clearer rollout semantics: disabled should mean "configured but not enabled", not "no configured provider" or "capability unsatisfied". Without that distinction, operator recovery and onboarding remain misleading.
- Recommended Action: Introduce a dedicated disabled-provider resolution/error path in binding resolution and surface that same semantics in preflight and repair guidance.

## Readiness Review

**Summary:** Not Ready

### READY-001 The focused gate is narrower than the proposal contract

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-004`, `REQ-007`, `REQ-010`
- Evidence Type: tests-run, code
- Evidence:
  - `scripts/test-gate.sh:128-131`
  - `scripts/test-gate.sh:1362-1371`
  - `examples/agents/agents.yaml:531-552`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:114-121`
- Why It Matters: `proposal-029` can pass while the authoritative catalog still lacks second-wave profiles and the registry layer is still Goose-specific. That makes the gate useful but insufficient as a readiness signal.
- Recommended Action: Expand the focused gate to assert canonical catalog presence, registry-neutral ownership, and disabled-provider semantics rather than only factory/routing/capability fragments.

### READY-002 Working tree is dirty, so this audit should not be treated as a pristine-release certification

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: none
- Evidence Type: inference
- Evidence:
  - `git status --short` at audit start showed `M "Chainworks ForgeTests/Proposal013Tests.swift"` and `?? docs/proposals/032-atomic-transition-settlement-and-durable-resume-cursor_IMPLEMENTATION_AUDIT_R1.md`
- Why It Matters: Same-tree proof still matters, but a dirty tree reduces handoff clarity and can hide unrelated drift around a proposal-readiness claim.
- Recommended Action: Re-run the focused gate on a clean tree before treating Proposal 029 as sign-off-ready.

## Verification Evidence

- `rg` / file inspection across provider platform, runtime transport, MCP policy, catalog YAML, report builder, and test-gate definitions.
- Same-tree gate execution:
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-029`
  - Result: `56 tests in 3 suites passed`, `TEST SUCCEEDED`
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-029-20260409-112620.xcresult`
- Additional Goose-path spot check:
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ResumeManagerTests/ExecutionService uses live executor for live workflow' CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=`
  - Result: selector resolved to `0 tests`; this audit therefore used direct code + tests-found evidence for Goose-path continuity instead of counting that command as runtime proof.

## Final Assessment

Proposal 029 has meaningful implementation progress, but it is still a partial platform slice rather than a finished second-wave ACP rollout. The factory safety work and proof gate are real. The proposal-owned transport-neutral registry migration, second-wave provider diagnostics/readiness ownership, and canonical catalog adoption are not. That keeps `Overall Conformance = Not Implemented` and `Overall Readiness = Not Ready`.
