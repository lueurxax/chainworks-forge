# Proposal 030: Second-Wave ACP Runtime Profiles Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `2d4bdc9` |
| Working Tree | clean |
| Audited At | `2026-04-09T17:08:14+0300` |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 030 has moved materially forward since `R3`. The second-wave transports are no longer stubs, the focused same-tree `proposal-030` gate is green, the provider-platform expansion is present, unknown adapter families fail closed, `RuntimeProfile.requires` is enforced, and disabled-provider rollout gating is wired through the runtime owner chain.

The proposal is still not fully implemented on the current tree for two narrow but proposal-owned reasons. First, the MCP registry/readiness path is only partially adapter-aware: executor dispatch now branches by family, but preflight, read-model preparation, bridge defaults, and install-validation logic still remain Goose-centric. Second, the amended acceptance contract now explicitly requires one successful same-tree execution proof path for each second-wave family (`codex_acp`, `auggie_cli_acp`, `junie_cli_acp`), and that proof is still absent. The current green gate proves structural coverage plus first-wave ACP execution, not second-wave execution completion.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | `REQ-013` is still missing, and `REQ-003` / `REQ-012` remain partial | High |
| Architecture | At Risk | MCP registry ownership is still split between adapter-aware and Goose-only paths | High |
| Product | At Risk | second-wave families can be configured and routed, but their full successful execution slice is still unproven | High |
| UI | Acceptable | readiness/preflight surfaces use generic copy while still loading Goose-specific registry sources | Medium |
| UX | At Risk | Codex rich-MCP and zero-MCP family semantics are not surfaced consistently from one canonical owner chain | Medium |
| Readiness | Not Ready | a green focused gate exists, but it does not satisfy the proposal's all-family successful-proof requirement | High |

## Proposal Contract

### Scope

- Expand the provider platform with `codexACP`, `auggie`, and `junie` families, adapters, seeded settings, capabilities, and health ownership.
- Keep current canonical catalog cutovers intact, including `codex_writer_high -> codex_acp`, Gemini review profiles, and second-wave ACP orchestrator profiles.
- Make transport selection fail closed for unknown adapter families.
- Finish MCP registry migration so second-wave runtime namespaces and readiness checks are adapter-family-aware rather than Goose-only.
- Enforce `RuntimeProfile.requires` through `ProviderCapabilities`.
- Deliver real runnable second-wave ACP transports plus proposal-owned proof coverage for each in-scope family.

### Locked Decisions

- `P030` is not catalog-only; it owns runtime execution work.
- Unknown adapter families must never silently downgrade to Goose.
- `RuntimeProfile.requires` extends `ProviderCapabilities`; it does not create a parallel capability authority.
- Codex ACP keeps the `codex` namespace and rich MCP mappings.
- Auggie and Junie are zero-MCP-only in `P030`.
- `ConfiguredProvider.isEnabled` is the single rollout gate.
- The phase plan is sequencing inside one proposal, not deferred out-of-scope future work.

### Primary User Flows

1. Operators can see second-wave providers in settings/readiness and enable them deliberately.
2. A run resolves onto the intended adapter family without silent Goose downgrade.
3. Preflight blocks unsupported adapter families, disabled providers, and invalid MCP/capability combinations before execution.
4. A run executes on the intended second-wave transport and preserves truthful runtime/binding identity into persisted snapshots and reports.

### Acceptance / Proof Requirements

Proposal 030 explicitly requires:

1. second-wave provider-platform expansion with adapters, capabilities, and health probes;
2. fail-closed transport selection and registered-family validation;
3. transport-neutral MCP registry ownership with explicit second-wave namespaces;
4. `requires` enforcement through `ProviderCapabilities`;
5. locked capability-token consumers;
6. preserved Goose default path;
7. truthful snapshots/reports across provider families;
8. single-owner rollout enablement through `isEnabled`;
9. a green same-tree `proposal-030` gate;
10. preserved canonical catalog rollout decisions;
11. no remaining second-wave stub transport failures;
12. deterministic family-specific MCP behavior;
13. one successful same-tree execution proof path for each in-scope second-wave family.

## Proposal Fidelity / Divergence

### Matches

- Second-wave `ProviderFamily` cases, adapter registrations, seeded capability defaults, and disabled seeded providers exist.
- Runtime transport selection is fail-closed for unknown adapter families.
- `RuntimeProfile.requires` is enforced through `ProviderCapabilities`.
- Preferred-provider resolution and repair now respect `isEnabled`.
- Canonical catalog cutovers remain in place.
- `CodexACPTransport`, `AuggieCLIACPTransport`, and `JunieCLIACPTransport` now implement session creation, prompt streaming, and close paths instead of throwing `"not yet implemented"`.
- The focused same-tree `proposal-030` gate now passes.

### Divergences

- Adapter-aware MCP registry migration is incomplete. The executor now dispatches by adapter family, but preflight, read-model preparation, bridge defaults, and installed-extension validation still remain Goose-first.
- Codex rich-MCP does not yet fail closed on a Codex-specific missing/unreadable registry source the way the proposal now requires.
- Same-tree successful execution proof remains first-wave only; no successful proof path exists yet for `codex_acp`, `auggie_cli_acp`, or `junie_cli_acp`.

### Ambiguities / Evidence Gaps

- This audit did not rely on live external Codex/Auggie/Junie environments; proof is same-tree code/test evidence only.
- Auggie/Junie zero-MCP semantics are materially present through missing runtime mappings and generic MCP policy resolution, but not yet expressed through one explicit family-owned registry/readiness source.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
| Partially Implemented | 3 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Second-wave provider-platform expansion exists

- Proposal Source: `§3`, `§4.1`, `§5.1`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-36`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:118-183`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-304`
  - `Chainworks Forge/Providers/ProviderAdapter.swift:23-33`
  - `Chainworks Forge/Providers/CodexACPProviderAdapter.swift:3-42`
  - `Chainworks Forge/Providers/AuggieProviderAdapter.swift:3-42`
  - `Chainworks Forge/Providers/JunieProviderAdapter.swift:3-42`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:129-181`
- Gap / Note: The second-wave families, adapters, seeded defaults, and health-verification owners are present on the current tree.

### REQ-002 Unknown adapter families fail closed and the owner chain surfaces the error before silent downgrade

- Proposal Source: `§4.2`, `§5.2`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Engine/ExecutionService.swift:1079-1113`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:52-55`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:147-155`
  - `Chainworks Forge/Engine/PreflightService.swift:205-224`
  - `Chainworks ForgeTests/Proposal029Tests.swift:11-84`
- Gap / Note: The factory throws `unknownAdapterFamily`, the executor turns that into agent failure instead of silent downgrade, and preflight rejects unknown families before run start.

### REQ-003 MCP registry ownership is transport-neutral and resolved through the correct adapter-family owner

- Proposal Source: `§4.3`, `§5.3`
- Status: Partially Implemented
- Evidence Type(s): `code`
- Evidence References:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:114-119`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:190-264`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49-66`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-489`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-29`
  - `Chainworks Forge/Views/IdeaListView.swift:2302-2309`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:311-317`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:392-399`
- Gap / Note: `RuntimeExtensionRegistrySnapshot`, second-wave runtime namespaces, and `CodexExtensionRegistryReader` exist, and executor-time dispatch is adapter-aware. But preflight still loads `GooseExtensionRegistryReader()` directly, read-model policy resolution still snapshots Goose directly, bridge defaults remain Goose-owned, and installed-extension validation still special-cases `runtimeNamespace == "goose"`. The adapter-aware registry migration is therefore incomplete.

### REQ-004 Preflight validates `RuntimeProfile.requires` through `ProviderCapabilities`

- Proposal Source: `§4.4`, `§5.4`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-304`
  - `Chainworks Forge/Engine/PreflightService.swift:632-665`
  - `Chainworks ForgeTests/Proposal029Tests.swift:122-133`
- Gap / Note: Capability validation is owned by `ProviderCapabilities.satisfies(_:)` and exercised in preflight; there is no second authority for `requires`.

### REQ-005 Every `requires` token maps to a locked capability field and consumer

- Proposal Source: `§4.4`, `§5.5`
- Status: Implemented
- Evidence Type(s): `code`
- Evidence References:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:241-304`
  - `examples/agents/agents.yaml:535-577`
  - `Chainworks Forge/Engine/PreflightService.swift:640-665`
- Gap / Note: The in-scope token vocabulary maps onto concrete capability fields with an enforcement consumer in preflight.

### REQ-006 Goose default path remains operational

- Proposal Source: `§3`, `§4.2`, `§5.6`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Engine/ExecutionService.swift:1079-1086`
  - `Chainworks ForgeTests/Proposal026Tests.swift:248-249`
- Gap / Note: Goose remains an explicit path only for `adapterFamily == "goose"` or missing bindings; the fail-closed changes do not regress the legacy default path.

### REQ-007 Run snapshots and execution reports preserve truthful provider/runtime identity across provider families

- Proposal Source: `§5.7`
- Status: Partially Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Models/Run.swift:43-57`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:883-892`
  - `Chainworks ForgeTests/Proposal026Tests.swift:476-485`
  - `Chainworks ForgeTests/Proposal026Tests.swift:592-601`
- Gap / Note: The persistence/report model is still wired for truthful runtime/binding identity and is proven on first-wave ACP execution paths. But the proposal now requires consistency across all provider families, and successful second-wave execution has not yet been proven through this path.

### REQ-008 Rollout enablement uses `ConfiguredProvider.isEnabled` as the single owner

- Proposal Source: `§4.8`, `§5.8`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift:3-36`
  - `Chainworks Forge/Providers/ProviderRegistry.swift:35-40`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:56-63`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift:158-181`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:96-149`
  - `Chainworks ForgeTests/Proposal029Tests.swift:137-188`
- Gap / Note: Disabled second-wave providers seed correctly, preferred-provider resolution ignores disabled providers, and preferred-provider repair now advances only to enabled same-family providers.

### REQ-009 Focused same-tree `proposal-030` gate passes

- Proposal Source: `§5.9`
- Status: Implemented
- Evidence Type(s): `tests-run`
- Evidence References:
  - `scripts/test-gate.sh:128-132`
  - `scripts/test-gate.sh:1150`
  - `scripts/test-gate.sh:1370-1379`
  - Command: `bash 'scripts/test-gate.sh' proposal-030`
  - Result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-030-20260409-170948.xcresult`
  - Outcome: `Test run with 62 tests in 3 suites passed after 12.065 seconds. ** TEST SUCCEEDED **`
- Gap / Note: The focused gate is green on the current tree. This closes the earlier red gate blocker, but it does not replace the proposal's separate all-family execution-proof requirement.

### REQ-010 Canonical catalog preserves the landed rollout decisions

- Proposal Source: `§3.1`, `§5.10`
- Status: Implemented
- Evidence Type(s): `code`
- Evidence References:
  - `examples/agents/agents.yaml:113-145`
  - `examples/agents/agents.yaml:553-577`
  - `examples/agents/agents.yaml:595-602`
  - `examples/agents/agents.yaml:719-726`
  - `examples/agents/agents.yaml:743-766`
  - `examples/agents/agents.yaml:1183-1216`
- Gap / Note: The current catalog still preserves Codex runtime mappings, `codex_writer_high -> codex_acp`, Gemini review profiles on `gemini_review_pro`, and second-wave ACP orchestrator profiles with `structured_output: preferred`.

### REQ-011 Runs routed to `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp` no longer fail with stub transport errors

- Proposal Source: `§3.2`, `§5.11`
- Status: Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:35-133`
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:135-282`
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift:282`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:35-133`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:135-282`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift:282`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:35-133`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:135-282`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift:282`
  - `Chainworks ForgeTests/Proposal029Tests.swift:192-256`
- Gap / Note: The second-wave transports now implement create/prompt/close paths. The current tests explicitly confirm that session creation failures are subprocess failures rather than `"not yet implemented"` stub failures.

### REQ-012 MCP behavior is deterministic per second-wave family

- Proposal Source: `§5.12`
- Status: Partially Implemented
- Evidence Type(s): `code`, `tests-found`
- Evidence References:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49-66`
  - `examples/agents/agents.yaml:113-145`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1848-1891`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-489`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:311-317`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:392-399`
- Gap / Note: Namespace mapping and Codex catalog mappings exist, and Auggie/Junie inherit effective zero-MCP behavior because requested MCP lanes lack runtime mappings for `auggie` / `junie`. But Codex registry/readiness does not yet fail closed through a Codex-owned registry path in preflight or resolver validation; the blocking behavior still special-cases Goose.

### REQ-013 Same-tree verification includes one successful execution proof path for each in-scope second-wave family

- Proposal Source: `§3.2`, `§5.13`
- Status: Missing
- Evidence Type(s): `tests-run`, `tests-found`, `inference`
- Evidence References:
  - `Chainworks ForgeTests/Proposal026Tests.swift:353-405`
  - `Chainworks ForgeTests/Proposal029Tests.swift:192-256`
  - `scripts/test-gate.sh:128-132`
  - `bash 'scripts/test-gate.sh' proposal-030` → `62 tests in 3 suites passed`
- Gap / Note: The current same-tree proof coverage is still first-wave only. `Proposal026Tests` executes successful proof paths for `claude_agent_acp` and `gemini_cli_acp`, while `Proposal029Tests` only proves non-stub session-creation failure behavior for Codex/Auggie/Junie. There is no successful executed proof path yet for any second-wave family.

## Expert Findings

### ARCH-001 Adapter-aware MCP registry migration remains split across multiple owners

- Severity: Major
- Confidence: High
- Related Proposal Items / REQs: `§4.3`, `REQ-003`, `REQ-012`
- Evidence Type(s): `code`
- Evidence References:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift:477-489`
  - `Chainworks Forge/Engine/PreflightService.swift:677-719`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:24-29`
  - `Chainworks Forge/Views/IdeaListView.swift:2302-2309`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:311-317`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:392-399`
- Why It Matters: Execution-time MCP registry selection has started to branch by adapter family, but preflight, bridge defaults, read-model preparation, and installed-extension validation still depend on Goose-specific loaders or Goose-only validation conditions. That leaves second-wave runtime readiness truth split across incompatible owners.
- Recommended Action: Move preflight, read-model policy resolution, bridge defaults, and installed-extension validation onto one adapter-aware registry provider contract and make Codex-specific registry absence block rich-MCP Codex runs the same way proposal 029 promises.

### PROD-001 Second-wave execution success is still not proven on the same tree

- Severity: Major
- Confidence: High
- Related Proposal Items / REQs: `§3.2`, `REQ-007`, `REQ-013`
- Evidence Type(s): `tests-run`, `tests-found`
- Evidence References:
  - `Chainworks ForgeTests/Proposal026Tests.swift:353-405`
  - `Chainworks ForgeTests/Proposal029Tests.swift:192-256`
  - `bash 'scripts/test-gate.sh' proposal-030` → `62 tests in 3 suites passed`
- Why It Matters: The current tree proves that second-wave transports are no longer stubs, but it still does not prove that a real run can execute successfully on Codex/Auggie/Junie while preserving runtime truth. That leaves the proposal's claimed user value only partially delivered.
- Recommended Action: Add one successful same-tree execution proof path for each of `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`, including runtime-profile and report truth assertions comparable to the first-wave proof tests.

### UI-001 Preflight/readiness copy is more generic than the actual MCP owner chain

- Severity: Minor
- Confidence: Medium
- Related Proposal Items / REQs: `REQ-003`, `REQ-012`
- Evidence Type(s): `code`
- Evidence References:
  - `Chainworks Forge/Engine/PreflightService.swift:677-710`
  - `Chainworks Forge/Views/IdeaListView.swift:2302-2309`
- Why It Matters: The UI-facing preflight check is labeled generically as `Runtime Extension Registry`, but the actual source still defaults to Goose-only snapshots in preflight and read-model preparation. That makes the surface look more transport-neutral than the underlying implementation really is.
- Recommended Action: Align user-facing readiness copy with the actual adapter-family registry owner, or complete the migration so the generic label is accurate.

### READY-001 Green focused gate is not yet sufficient for proposal sign-off

- Severity: Critical
- Confidence: High
- Related Proposal Items / REQs: `REQ-009`, `REQ-013`
- Evidence Type(s): `tests-run`, `tests-found`
- Evidence References:
  - `scripts/test-gate.sh:128-132`
  - `scripts/test-gate.sh:1370-1379`
  - `Chainworks ForgeTests/Proposal026Tests.swift:353-405`
  - `Chainworks ForgeTests/Proposal029Tests.swift:192-256`
- Why It Matters: The gate is green, but its proof mix still combines first-wave successful execution with second-wave structural tests. That is not enough to satisfy the proposal's amended proof contract for second-wave transport completion.
- Recommended Action: Keep `proposal-030` red at proposal-sign-off level until the gate includes one successful execution proof path per in-scope second-wave family.

## Audit Evidence

### Focused Gate

- Command: `bash 'scripts/test-gate.sh' proposal-030`
- Outcome: `** TEST SUCCEEDED **`
- Summary: `62 tests in 3 suites passed after 12.065 seconds`
- Result Bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-030-20260409-170948.xcresult`

### Additional Inspection

- Proposal contract extracted from `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie.md`
- Targeted code inspection of provider-platform, transport, MCP-policy, preflight, bridge, read-model, and report surfaces
- Targeted test inspection of `Proposal029Tests`, `Proposal026Tests`, and `ProviderPlatformTests`

## Recommended Next Actions

1. Finish the adapter-aware MCP registry migration across preflight, bridge defaults, read-model policy resolution, and installed-extension validation.
2. Add successful same-tree execution proof paths for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`.
3. Re-run `bash 'scripts/test-gate.sh' proposal-030` after those proofs land and only then re-audit for implementation sign-off.
