# Proposal 033: Remove Goose From Canonical Transport And Simplify Runtime Dispatch

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Codex / Andrey Khasanov |
| Depends on | [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/goose-server-transport.md](../reference/goose-server-transport.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Remove Goose as the canonical runtime transport implementation after second-wave ACP providers are proven, and simplify runtime dispatch to an ACP-first core with Goose retained only as explicit optional compatibility adapter. |
| Goal | Replace the current multi-implementation transport shape with a single canonical ACP execution seam and capability-gated provider adapters. |

---

## 1. Context and Motivation

The current runtime baseline established ACP-shaped transport as the canonical model but kept Goose as the default runtime. Proposal 030 introduces the next runtime expansion: Codex ACP, Auggie CLI ACP, and Junie CLI ACP.

Once those providers are implemented and proven, we should complete the migration by removing Goose from the canonical runtime path so that:

- runtime selection is no longer coupled to Goose defaults,
- transport code in core is reduced to ACP concepts only,
- Goose lives only as an adapter compatibility entry point, not as a model for future implementation.

---

## 2. Hard Prerequisite Gate

**This proposal cannot begin implementation until P030 passes its full audit to `Implemented / Ready`.**

Enforcement: the `proposal-033` gate in `test-gate.sh` must assert P030 readiness before running any P033-specific tests. If the P030 focused gate is not green, the P033 gate fails immediately.

```bash
# In test-gate.sh, proposal-033 preamble:
log "Prerequisite: proposal-030 must be green"
run_targeted_tests "proposal-030-prereq" "${PROPOSAL_030_TESTS[@]}"
```

This is a fail-closed dependency, not a narrative prerequisite.

---

## 3. Product Questions This Proposal Must Answer

After completion, the system should answer:

1. Can `Chainworks Forge` execute runs with Goose removed from canonical transport code while preserving existing truth contracts?
2. Can transport selection remain catalog-driven through `runtime_profile` and `backend_profile` only?
3. Can default runtime behavior remain stable after removing Goose as canonical default?
4. Can adapter-specific compatibility code for Goose be isolated and removed from core orchestration/test assumptions?
5. Can existing proof lanes be preserved while replacing transport complexity with a simpler dispatch model?
6. Does the operator see clear migration guidance for each surface that changes from Goose-first to ACP-first?
7. Does the trust/remediation model work for ACP-default runs without Goose-specific vocabulary?

---

## 4. Scope

This proposal includes:

- hard deprecation of Goose as canonical transport implementation in core orchestration
- canonical runtime factory rewrite to instantiate ACP adapter families through one unified transport pathway
- removal of Goose-only lifecycle assumptions from execution orchestration and transport abstractions
- explicit "compatibility adapter" retention for Goose where needed
- migration of any remaining runtime-profile references away from `goose_rest_sse` as default mandatory path
- **MCP schema migration** (not removal): `mcp_profile` and `mcp_server_registry` transition to `backend_profile`-declared MCP intent with explicit migration path
- updating failure evidence and proof expectations for transport simplification
- **explicit core/compatibility ownership split** for every Goose-touching file
- **operator surface migration plan** for each Goose-first UI surface
- **ACP-default trust model** defining trust states for ACP runs without Goose vocabulary
- **`proposal-033` gate** in `test-gate.sh` with P030 prerequisite check

This proposal does not include:

- removing Goose app support from operator workflows entirely
- deleting Goose binary launch tooling from all system-level settings
- relaxing execution/recovery/report truth guarantees

---

## 5. Design

### 5.1 Canonical seam is ACP-only

Core runtime execution should contain only ACP-shaped abstractions:

- session lifecycle
- prompt streaming
- cancel/close
- state inspection where available
- capability discovery
- session update/mode/model mutation events

No direct Goose endpoint semantics (e.g., `/agent/start`, `/agent/update_provider`) remain in core transport code.

### 5.2 Transport dispatch simplification

Replace current mixed transport selection with:

- one canonical runtime dispatch entry (`ACPRuntimeTransport` / equivalent)
- adapter registration by `adapter_family`
- strict capability checks before launch
- provider-specific execution adapters implement only transport-facing mechanics

Expected mapping:

- `provider != claude_agent_acp|gemini_cli_acp|codex_acp|auggie_cli_acp|junie_cli_acp`: rejected by catalog/preflight
- Goose runtime family can still exist under a compatibility adapter namespace if needed for migration fallback

### 5.3 Core / Compatibility Ownership Split

Every Goose-touching file must be classified as either **core** (stays, renamed to transport-neutral) or **compatibility** (moved to `GooseAdapter/` or `GooseCompat/`):

| File | Current Role | Post-P033 Classification |
|------|-------------|-------------------------|
| `RuntimeTransport.swift` | Core protocol | **Core** — already transport-neutral |
| `ExecutionService.swift` (transport factory) | Core dispatch | **Core** — factory switch stays, Goose case becomes compat-only |
| `GooseAgentExecutor.swift` → `RuntimeAgentExecutor` | Core executor | **Core** — rename to `RuntimeAgentExecutor.swift`, remove Goose-specific naming |
| `GooseSessionBridge.swift` → `RuntimeSessionBridge.swift` | Core session bridge | **Core** — already used by ACP, rename |
| `GooseServerTransport.swift` | Goose transport | **Compatibility** — stays in `GooseAdapter/` |
| `GooseServerManager.swift` | Goose server lifecycle | **Compatibility** — stays in `GooseAdapter/` |
| `GooseStreamEventMapper.swift` | Goose SSE parsing | **Compatibility** — stays in `GooseAdapter/` |
| `GooseTransport.swift` | Bespoke Goose transport | **Compatibility** — remove or archive |
| `MCPPolicyRuntime.swift` (GooseExtensionRegistryReader) | Goose MCP reader | **Compatibility** — stays as Goose-specific conformer |
| `FixtureGooseTransport.swift` | Test fixture transport | **Core** — rename to `FixtureRuntimeTransport.swift`, remove Goose naming |
| `GooseProviderConnectionAssistant.swift` | Goose setup assistant | **Compatibility** — move to `GooseCompat/`, guard behind Goose family check |
| `GooseProviderConnectionAssistantView.swift` | Goose setup UI | **Compatibility** — move to `GooseCompat/`, only shown when Goose family selected |
| `GooseAgentExecutorTests.swift` | Executor tests | **Core** — rename to `RuntimeAgentExecutorTests.swift` |
| `GooseSessionBridgeTests.swift` | Bridge tests | **Core** — rename to `RuntimeSessionBridgeTests.swift` |
| `GooseServerTransportTests.swift` | Goose transport tests | **Compatibility** — stays alongside Goose transport |
| `GooseStreamEventMapperTests.swift` | Goose mapper tests | **Compatibility** — stays alongside Goose mapper |
| `GooseServerLiveIntegrationTests.swift` | Live Goose proof | **Compatibility** — guard behind Goose availability |
| `PilotReadinessView.swift` (Goose readiness) | Goose-first readiness | **Core** — migrate to adapter-neutral readiness language |
| `RunsHomeView.swift` (trust rendering) | `server_verified` display | **Core** — update to read new + legacy trust values |

### 5.4 MCP Schema Migration

**Current state**: The MCP contract is richer than just "required servers." Current canonical truth includes:

- `mcp_profile` per agent — declares required and optional extensions, fallback policy (`AgentCatalog.swift:395`)
- `mcp_server_registry` — maps server IDs to runtime-specific extension IDs per namespace (`AgentCatalog.swift`)
- Resolved MCP report with requested/predicted/actual/denied layers (`MCPPolicyRuntime.swift:267`)
- Frozen `resolvedMCPPoliciesJSON` on `Run` for immutable provenance
- Report surfaces that render the four-layer MCP truth (`RunReportBuilder.swift:196`)

**This proposal does NOT delete them in one step.** The replacement must preserve the same truth granularity.

**Phase 1 — `mcp_intent` on `backend_profile` (dual-path)**:

`mcp_intent` is not just "required servers." It carries the same contract as current `mcp_profile`:

```yaml
backend_profiles:
  codex_orchestrator_acp:
    provider: codex_acp
    model: gpt-5
    runtime_profile: codex_acp
    mcp_intent:
      required: [context7, filesystem]
      optional: [web_search]
      fallback_policy: fail_if_required_missing
```

Preflight validates `mcp_intent.required` against machine-local capability (adapter-specific registry provider). The resolved four-layer truth (requested → predicted → actual → denied) continues to be computed by `MCPPolicyResolver` and frozen on `Run`.

Old `mcp_profile` / `mcp_server_registry` path continues working unchanged for Goose-backed runs.

**Phase 1 survival contract** — these pieces stay intact:
- `MCPPolicyResolutionReport` with all four layers
- Frozen `resolvedMCPPoliciesJSON` on `Run`
- Report/comparison surfaces rendering MCP truth
- `mcp_server_registry` for runtime-namespace-to-extension-ID mapping

Only the **input declaration point** moves from agent-level `mcp_profile` to backend-profile-level `mcp_intent`.

**Phase 2 — Deprecation**: New catalog entries use `mcp_intent` only. Old paths emit deprecation warnings. Validator warns if `mcp_profile` is used on agents whose backend profile already declares `mcp_intent`.

**Phase 3 — Removal**: After all catalog entries migrated, remove `mcp_profile` from agent schema. `mcp_server_registry` stays if any adapter still needs runtime-namespace mapping; otherwise it moves to machine-local config.

Each phase has its own acceptance test in the `proposal-033` gate.

### 5.5 Goose as compatibility adapter, not canonical path

Goose-related code in implementation should be moved to compatibility-only modules, including:

- Goose session-specific extension reconciliation bridge
- local `goosed` bootstrapping/probe helpers
- Goose extension IDs and response normalization that are no longer needed for new provider families

These modules should:

- be clearly marked compatibility-only
- have narrower surface
- remain behind profile gating and explicit diagnostics flags

### 5.6 Runtime-profile and backend-profile migration

After this proposal:

- `runtime_profiles` for default operation should not point to Goose unless explicitly selected by operator choice
- catalog defaults should shift to one of the implemented ACP profiles as first option
- `backend_profile` remains the run-time contract anchor: provider + model + runtime profile

### 5.7 Operator Surface Migration

Each Goose-first UI surface must change. Explicit migration plan:

| Surface | Current State | Post-P033 State |
|---------|--------------|-----------------|
| `ProviderSettingsView.swift` | Goose transport as primary setup path | ACP-first setup wizard; Goose under "Advanced / Compatibility" |
| `FirstRunSetupWizard.swift` | Goose server configuration as default | ACP provider selection as default; Goose as optional |
| `IdeaListView.swift` (runtime readiness) | "Goose server" readiness language | "Runtime" readiness language; adapter-family-specific status |
| `PilotReadinessView.swift` | Goose-first readiness checks | Adapter-neutral readiness; per-family health status |
| `RunsHomeView.swift` (trust/provenance) | `server_verified` / `server_unverified` display | Normalized trust vocabulary with legacy fallback |
| `operator-experience.md` | Goose-first onboarding docs | ACP-first onboarding; Goose in compatibility section |
| Preflight messages | "Goose extension registry" | "Runtime extension registry" (already done in P029) |
| Error remediation | Goose-specific recovery suggestions | Adapter-family-specific recovery (e.g., "Check codex-acp binary") |

### 5.8 ACP-Default Trust Model

Current provenance vocabulary uses `fixture_verified | server_unverified | server_verified` which is Goose-centric. Post-P033 trust states:

| Trust State | Meaning | Applies To |
|-------------|---------|-----------|
| `fixture_verified` | Run used fixture transport (test) | All adapters |
| `runtime_unverified` | Live run, adapter responded but capability not fully proven | ACP second-wave |
| `runtime_verified` | Live run, adapter fully proven with canonical proof | ACP first-wave, Goose |
| `compatibility_fallback` | Run used Goose compatibility adapter | Goose-backed runs post-P033 |

`runtimeTrustLevel` on `Run` is updated to use these states. Report/recovery surfaces read the trust level to provide appropriate remediation language.

**Forward compatibility for historical runs**: Existing persisted runs use `server_unverified` and `server_verified`. These values must not become ambiguous. The migration adds a reader fallback:

| Persisted Value | Post-P033 Interpretation |
|----------------|-------------------------|
| `fixture_verified` | `fixture_verified` (unchanged) |
| `server_unverified` | `runtime_unverified` (Goose-era, equivalent meaning) |
| `server_verified` | `runtime_verified` (Goose-era, equivalent meaning) |
| `nil` | `nil` (pre-trust-level runs, no change) |

Implementation: `Run.runtimeTrustLevel` getter normalizes legacy values on read. The raw persisted string is not mutated — only the reader maps old → new. Shell/report surfaces (`RunsHomeView.swift:637`, preview fixtures) use the normalized value.

This ensures old runs display correctly in the new vocabulary without a data migration.

### 5.9 Documentation and evidence

Update evidence and proposal-index docs to mark this as "Goose transport canonical path retired."

---

## 6. Rollout Sequence

1. **P030 prerequisite**: Complete Proposal 030 proof lane for all second-wave providers. `proposal-033` gate asserts P030 green before proceeding.
2. Add `proposal-033` gate to `test-gate.sh` with P030 prerequisite and Phase 1 MCP migration tests.
3. Implement Phase 1 MCP dual-path (`mcp_intent` on `backend_profile`).
4. Implement core/compatibility ownership split (renames, module moves).
5. Implement operator surface migration (Settings, Setup Wizard, readiness language).
6. Implement ACP-default trust model.
7. Freeze default runtime selection against non-Goose ACP profile in the catalog.
8. Implement Phase 2 MCP deprecation.
9. Run focused proof gates on all phases.
10. Implement Phase 3 MCP removal (only after Phase 2 is proven).
11. Decommission Goose-as-default in tests and examples.

---

## 7. Acceptance Criteria

Proposal 033 is complete when:

1. `proposal-033` gate exists in `test-gate.sh` with a fail-closed P030 prerequisite check.
2. Core orchestration transport has no Goose-shaped canonical assumptions.
3. Runtime dispatch is uniform and ACP-first.
4. Every Goose-touching file is classified as core (renamed) or compatibility (in `GooseAdapter/`/`GooseCompat/`).
5. Goose is retained only as optional compatibility adapter code, not canonical default.
6. `backend_profile` + `runtime_profile` selection works for the non-Goose default and at least one additional ACP provider.
7. Canonical execution truth (`RunStartSnapshot`, `AgentExecution`, report/recovery readers) remains unchanged in semantics and fidelity.
8. MCP schema migration follows the three-phase plan: dual-path → deprecation → removal.
9. `mcp_intent` on `backend_profile` is validated directly against machine-local MCP capability.
10. Operator-facing surfaces (Settings, Setup Wizard, readiness, error remediation) are ACP-first with Goose under compatibility.
11. `runtimeTrustLevel` uses the post-P033 vocabulary (`runtime_unverified`, `runtime_verified`, `compatibility_fallback`).
12. Operator-facing docs and onboarding no longer imply Goose is the canonical runtime shape.
13. Test and proof lanes pass for the post-removal transport shape with explicit, documented fallback if Goose adapter remains enabled for compatibility.

---

## 8. Risks

- Migration risk: hidden Goose assumptions remain in core code paths and cause runtime regressions.
- Capability mismatch risk: one or more ACP providers may regress and force temporary backtracking in default selection.
- Onboarding risk: if Goose is demoted, local operator workflows that still rely on Goose tooling may require clearer migration guidance.
- MCP migration risk: phased removal may leave inconsistent state if phases are not completed in order.

---

## 9. Alternatives Considered

### 9.1 Keep Goose as canonical with translation adapters

Rejected. This retains avoidable coupling and delays the core simplification target.

### 9.2 Remove Goose entirely from codebase

Rejected for this phase. Compatibility support remains valuable for environments still using Goose.

### 9.3 Defer simplification until a fourth provider lands

Rejected. Once three providers are proven, transport complexity is high relative to remaining benefit; this is the right time for simplification.

### 9.4 Remove `mcp_profile`/`mcp_server_registry` in one step

Rejected by review. These types have consumers in DSL, runtime, and reporting. A three-phase migration (dual-path → deprecation → removal) is safer and allows incremental proof.

### 9.5 Skip operator surface migration

Rejected by review. Goose-first UI language persists in Settings, Setup Wizard, readiness messages, and docs. Post-P033 operator experience must be ACP-first to match the underlying transport reality.
