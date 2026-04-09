# Proposal 030: ACP Second-Wave Runtime Profiles Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `bc924c3` |
| Working Tree | dirty |
| Audited At | `2026-04-09T15:36:51+03:00` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 030 is materially closer to implemented than in R1, but it is still not proposal-complete on the current tree. The second-wave provider platform, capability mapping, disabled-provider rollout gate, and canonical catalog/runtime-profile data are now substantially in place. The remaining blockers are concentrated in two areas: MCP registry migration is still only partially transport-neutral, and the proposal-required focused `proposal-030` gate is red on the current tree because the test target fails to build. That keeps the slice out of `Implemented` / `Ready` status.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | same-tree `proposal-030` gate is red | High |
| Architecture | At Risk | registry abstraction stops at type rename and still has a Goose-first owner chain | High |
| Product | At Risk | second-wave ACP profiles still cannot claim rich MCP parity from the canonical catalog | High |
| UI | At Risk | operator-facing MCP/readiness language is still Goose-first in preflight | Medium |
| UX | At Risk | rollout semantics improved, but the operator still gets mixed runtime-neutral vs Goose-specific remediation | Medium |
| Readiness | Not Ready | targeted gate fails before it can prove the slice on the current tree | High |

## Proposal Contract

### Scope

- Expand the provider platform with `codexACP`, `auggie`, and `junie`.
- Make runtime transport selection fail-closed for unknown ACP adapter families.
- Migrate MCP registry ownership from Goose-specific terms to transport-neutral runtime registry ownership.
- Enforce `RuntimeProfile.requires` through `ProviderCapabilities` as the single capability authority.
- Ship canonical catalog/runtime-profile entries for the second wave.
- Gate rollout through `ConfiguredProvider.isEnabled`.

### Locked Decisions

- This is not a catalog-only slice; it expands the provider platform itself.
- Unknown `adapterFamily` values must never silently fall back to Goose.
- `RuntimeProfile.requires` extends `ProviderCapabilities`; it does not create a second authority.
- Goose remains the default continuity path for `adapterFamily == "goose"`.
- `ConfiguredProvider.isEnabled` is the single rollout gate.

### Primary User Flows

1. The operator sees second-wave providers in Settings, with disabled-by-default rollout state.
2. Backend/runtime-profile resolution selects Codex/Auggie/Junie without silent Goose fallback.
3. Preflight blocks unsupported or disabled second-wave bindings before run start.
4. Reports and persisted run truth preserve which provider/runtime-profile actually executed.

### UI Commitments

- Settings/readiness surfaces show second-wave providers as configured-but-disabled by default.
- Preflight distinguishes provider rollout gating from capability mismatch.

### UX Commitments

- Operators get actionable preflight failures instead of silent downgrade to Goose.
- Runtime/MCP resolution should follow the actual adapter family, not Goose-only assumptions.

### Acceptance Criteria

The proposal requires:

1. new provider families plus seeded settings, adapters, capabilities, and health probes;
2. fail-closed transport factory behavior for unknown adapter families, with preflight validation before run start;
3. transport-neutral MCP registry ownership and explicit ACP runtime namespaces;
4. preflight validation of `RuntimeProfile.requires` through `ProviderCapabilities`;
5. every `requires` token mapped to a locked capability field/consumer;
6. Goose path remains operational;
7. run snapshots and reports preserve truth across provider families;
8. rollout enablement uses `ConfiguredProvider.isEnabled`, including distinct "not enabled" handling and disabled-safe repair;
9. focused `proposal-030` gate passes on the same tree.

### Test / Evidence Requirements

- Same-tree focused `proposal-030` gate.
- Code/test evidence for provider platform expansion, namespace migration, capability enforcement, and rollout semantics.

### Explicit Exclusions

- No hard cutover away from Goose.
- No claim of cross-provider MCP parity beyond what the runtime/catalog path can actually prove.
- No requirement in this proposal to fully implement each ACP subprocess transport.

## Proposal Fidelity / Divergence

### Matches

- Second-wave `ProviderFamily` cases exist.
- `ProviderAdapterFactory` now includes second-wave adapters.
- `ProviderCapabilities` now includes `supportsMCPReconciliation`.
- `ProviderCapabilities.satisfies(...)` maps `mcp_reconciliation`.
- `BackendProfileResolverV2` now distinguishes `providerNotEnabled`.
- `PreflightService` now surfaces `Provider Not Enabled`.
- Canonical `examples/agents/agents.yaml` now includes second-wave runtime profiles and backend profiles.
- The transport factory still throws for unknown adapter families.

### Divergences

- MCP registry migration is incomplete: the type rename happened, but the concrete owner chain is still Goose-first.
- Canonical `mcp_server_registry` still does not map rich MCP lanes for second-wave runtime namespaces like `codex`, `auggie`, or `junie`.
- `ProviderSettingsStore.removeProvider(...)` still repairs preferred-provider IDs without filtering disabled entries.
- The focused `proposal-030` gate is red on the current tree.

### Ambiguities / Evidence Gaps

- I did not establish same-tree live runtime proof for second-wave ACP execution itself; the proposal did not require full transport implementation, so this remains outside the main conformance boundary.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 2 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Provider-platform expansion exists for second-wave families
- Proposal Source: §3, §4.1, §5.1
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-299`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:129-210`
  - `Chainworks Forge/Providers/ProviderAdapter.swift:23-33`
  - `Chainworks Forge/Providers/CodexACPProviderAdapter.swift:3-42`
  - `Chainworks Forge/Providers/AuggieProviderAdapter.swift:3-42`
  - `Chainworks Forge/Providers/JunieProviderAdapter.swift:3-42`
- Gap / Note: Families, seeded settings, capability defaults, and health-adapter ownership now exist on the provider platform.

### REQ-002 Unknown adapter families fail closed and preflight validates registration before run start
- Proposal Source: §4.2, §5.2
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1079-1110`
  - `Chainworks Forge/Engine/PreflightService.swift:205-214`
  - `Chainworks ForgeTests/Proposal029Tests.swift:11-48`
- Gap / Note: The factory throws `RuntimeTransportError.unknownAdapterFamily`, and preflight blocks unregistered adapter families before execution.

### REQ-003 MCP registry ownership is transport-neutral and resolves against the correct runtime owner
- Proposal Source: §4.3, §5.3
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift:243-249`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:37-119`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:121-187`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:190-325`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-30`
  - `Chainworks Forge/Engine/PreflightService.swift:669-748`
- Gap / Note: `RuntimeExtensionRegistrySnapshot` now exists, but the only concrete registry provider is still `GooseExtensionRegistryReader`, resolver/plumbing variable names are still Goose-specific, and preflight still presents the surface as `Goose Extension Registry`.

### REQ-004 Preflight validates `RuntimeProfile.requires` through `ProviderCapabilities`
- Proposal Source: §4.4, §5.4
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-303`
  - `Chainworks Forge/Engine/PreflightService.swift:632-667`
  - `Chainworks ForgeTests/Proposal029Tests.swift:122-133`
- Gap / Note: The runtime-profile capability vocabulary now resolves through the existing `ProviderCapabilities` owner path.

### REQ-005 Every `requires` token maps to a locked capability field/consumer
- Proposal Source: §4.4, §5.5
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:249-300`
  - `Chainworks Forge/Engine/PreflightService.swift:645-665`
- Gap / Note: `supportsMCPReconciliation` is now present, and `satisfies(_:)` includes `mcp_reconciliation`.

### REQ-006 Canonical catalog ships second-wave runtime profiles and backend profiles
- Proposal Source: §4.6, §5.1
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:531-573`
  - `examples/agents/agents.yaml:743-762`
- Gap / Note: The authoritative example catalog now contains `codex_acp`, `auggie_cli_acp`, `junie_cli_acp`, and corresponding backend profiles.

### REQ-007 Goose default path remains operational
- Proposal Source: §4.3, §5.6
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Engine/ExecutionService.swift:1079-1086`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:1212-1268`
- Gap / Note: Goose is still the explicit path for `adapterFamily == "goose"`, and the tree still carries a targeted live-executor routing test for that path.

### REQ-008 Rollout enablement uses `ConfiguredProvider.isEnabled` as the single owner
- Proposal Source: §4.8, §5.8
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:96-150`
  - `Chainworks Forge/Engine/PreflightService.swift:148-158`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:55-64`
- Gap / Note: Filtering and explicit `providerNotEnabled` semantics now exist, but preferred-provider repair still picks the next same-family provider without checking `isEnabled`.

### REQ-009 Focused `proposal-030` gate passes on the same tree
- Proposal Source: §5.9
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-030`
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-030-20260409-153926.xcresult`
  - Failure text: `Cannot find 'YAMLAgentCatalogLoader' in scope`; `Failed to produce diagnostic for expression`; `** TEST FAILED **`
- Gap / Note: The proposal’s required same-tree proof gate is red on the current tree, so the proposal cannot roll up to `Implemented` or `Ready`.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Registry migration stops at the type layer
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RuntimeTransport.swift:247-248`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:121-187`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:195-248`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:26-30`
- Why It Matters: The proposal locked transport-neutral registry ownership, but the current code still has a single Goose-specific reader and Goose-shaped resolver wiring. That keeps the abstraction half-migrated and makes second-wave MCP behavior dependent on Goose assumptions.
- Recommended Action: Finish the owner migration: rename the resolver parameters away from Goose semantics, add non-Goose registry-provider conformers or an explicit neutral registry owner, and route preflight/bridge through that owner.

### ARCH-002 Disabled-provider repair still leaks a stale preference owner
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-008`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:55-64`
- Why It Matters: Proposal 030 explicitly extended rollout ownership beyond selection into repair semantics. The current remove/repair path can still select a disabled provider as the new preferred provider for a family.
- Recommended Action: Filter repair candidates by `isEnabled` the same way `ProviderRegistry.preferredProvider(for:)` does.

## Product Review

**Summary:** At Risk

### PROD-001 Second-wave rich MCP lanes are still not represented in the canonical registry
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`
- Evidence Type: code, tests-found
- Evidence:
  - `examples/agents/agents.yaml:113-144`
  - `Chainworks ForgeTests/Proposal029Tests.swift:190-199`
- Why It Matters: The second-wave runtime profiles now exist, but the canonical `mcp_server_registry` still maps rich MCP lanes like `developer`, `analyze`, `xcode`, and `context7` only for first-wave namespaces. That means the second-wave slice is still narrower than the proposal implies.
- Recommended Action: Add explicit runtime-ID mappings for the second-wave ACP namespaces where the product intends those servers to be usable, then prove the mappings in the focused gate.

## UI Review

**Summary:** At Risk

### UI-001 Preflight still presents MCP readiness as Goose-specific
- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:677-711`
- Why It Matters: The operator-facing preflight surface is part of the rollout contract. Showing `Goose Extension Registry` after the proposal migrated to runtime-neutral language is a direct clarity mismatch.
- Recommended Action: Rename the preflight MCP readiness surface to runtime-neutral language and only mention Goose when the selected runtime namespace is actually Goose.

## UX Review

**Summary:** At Risk

### UX-001 Rollout remediation is mixed: provider gating is clear, MCP remediation is still Goose-first
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `REQ-003`, `REQ-008`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:150-158`
  - `Chainworks Forge/Engine/PreflightService.swift:693-708`
- Why It Matters: Operators now get a clear `Provider Not Enabled` message, which is good. But once they hit MCP validation, the messaging drops back to Goose-only semantics. That breaks the proposal’s goal of a coherent second-wave rollout story.
- Recommended Action: Keep the same operator language standard across rollout, capability, and MCP checks by keying messages off the resolved runtime namespace.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The mandatory same-tree proof gate is red
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-009`
- Evidence Type: tests-run
- Evidence:
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-030`
  - `Result bundle: /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-030-20260409-153926.xcresult`
  - `Cannot find 'YAMLAgentCatalogLoader' in scope`
  - `Failed to produce diagnostic for expression`
  - `** TEST FAILED **`
- Why It Matters: Proposal 030 explicitly requires a green focused gate on the same tree. The current tree fails before the tests can certify the slice.
- Recommended Action: Fix the broken `Proposal029Tests` build lane first, then rerun the focused gate before making any stronger readiness claim.

### READY-002 Current proof only reaches partial implementation confidence
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-003`, `REQ-009`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:121-187`
  - `examples/agents/agents.yaml:113-144`
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-030`
- Why It Matters: Even after the current implementation progress, the remaining gaps are not cosmetic. They affect proofability and the operator-facing rollout path.
- Recommended Action: Treat Proposal 030 as a near-complete but still blocked implementation slice, not as sign-off-ready work.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `proposal-030` build gate reached `** BUILD SUCCEEDED **` before test phase |
| Core provider/runtime flow validated | Partial | code paths inspected; targeted gate failed before certifying the slice |
| Empty/loading/error states covered | Not Checked | outside this proposal’s main scope |
| Accessibility risk acceptable | Not Checked | not a primary scope area for this audit |
| Localization risk acceptable | Not Checked | not a primary scope area for this audit |
| Critical tests executed | Partial | focused gate executed, but test target build failed |
| Full regression suite / canonical full gate passed on same tree/HEAD | Fail | proposal-specific gate is red; no broader green same-tree regression was available for a successful verdict |
| Privacy/permissions/entitlements reviewed | Not Checked | not proposal-critical here |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `python3 '/Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie.md'`
- `rg -n "superseded|deprecated|replaced by|obsolete" ...`
- focused code inspection across:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift`
  - `Chainworks Forge/Providers/ProviderAdapter.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `examples/agents/agents.yaml`
  - `Chainworks ForgeTests/Proposal029Tests.swift`
- same-tree gate execution:
  - `bash '/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh' proposal-030`
  - result: build phase green, test phase red, result bundle at `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-030-20260409-153926.xcresult`

## Recommended Next Actions

1. Fix the broken `Proposal029Tests` build lane and get `proposal-030` green on the current tree.
2. Complete the registry-owner migration so MCP readiness is genuinely runtime-neutral rather than Goose-first with renamed types.
3. Add second-wave runtime-ID mappings in `mcp_server_registry` for the MCP lanes the product intends to support, then prove them in the focused gate.
4. Update `ProviderSettingsStore.removeProvider(...)` so preferred-provider repair respects `isEnabled`.

