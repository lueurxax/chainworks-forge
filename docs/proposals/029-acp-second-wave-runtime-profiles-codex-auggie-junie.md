# Proposal 029: Add Codex, Auggie, and Junie ACP Profiles as Second-Wave Runtime Expansion

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Codex |
| Depends on | [026-acp-runtime-plan-additive-profiles.md](026-acp-runtime-plan-additive-profiles.md), [../reference/acp-runtime-transport.md](../reference/acp-runtime-transport.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Add a second-wave runtime profile and adapter expansion: Codex ACP, Auggie CLI ACP, Junie CLI ACP, while keeping the current ACP runtime transport invariants. |
| Goal | Expand runtime choice through catalog-backed profiles with strict preservation of execution truth and existing operator surfaces. |

---

## 1. Context and Motivation

The current ACP runtime transport baseline already completed the first structural migration:

- ACP-shaped core transport vocabulary
- Goose preserved as default continuity path
- two-wave candidate selection through runtime profiles and backend profiles

Research has now advanced enough to justify one clearly bounded second expansion wave.

From the current runtime-transport baseline, the top intended candidates for this wave are:

1. **Codex ACP** (ranked above Auggie and Junie by current replay/session evidence)
2. **Auggie CLI ACP**
3. **Junie CLI ACP**

This proposal makes those three explicit second-wave providers and sets the rollout contract for implementation.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system should be able to answer:

1. Can catalog/runtime-profile selection support at least three additional real ACP runtimes without changing the current transport seam?
2. Can `runtime_profile` and `backend_profile` ownership stay in one catalog truth model for all three providers?
3. Can preflight classify these providers with capability classes instead of claiming uniform parity?
4. Can reports/recovery/mcp-truth continue to use persisted Forge truth while adapter-specific details vary by provider family?
5. Can the second-wave add-on be done without altering the current default Goose runtime behavior?

---

## 3. Scope

This proposal includes:

- add catalog/runtime profile definitions for:
  - `codex_acp`
  - `auggie_cli_acp`
  - `junie_cli_acp`
- add backend-profile variants that target these runtime profiles and preserve existing provider model/mode fields
- add adapter-specific realization contracts in the same style as the current ACP runtime transport baseline (ACP-shaped runtime contract + vendor-specific layer)
- add provider-specific capability declarations to support safe rollout and preflight gating
- extend test/proof expectation language for each provider profile
- document rollout order and blocking criteria for operator-grade claims

This proposal does not include:

- changing Goose default status
- introducing a hard runtime cutover
- collapsing provider classes to a single implied parity tier
- asserting tool/permission/MCP completeness where evidence is incomplete

---

## 4. Design

### 4.1 Profile-first expansion

Each provider in this proposal is introduced through catalog data only:

- `runtime_profile` declares transport family and capability class
- `backend_profile` binds provider/model/effort/temperature and selected runtime profile
- `RunStartSnapshot` captures provider selection for persisted truth
- `AgentExecution` retains actual effective runtime family used per attempt

No per-agent local `acp_server` field is introduced. Backend profile remains the binding point.

### 4.2 Candidate mapping for this wave

- **Codex ACP**
  - Strong: `session/new`, `session/list`, `session/load`, session mutation streaming, usage telemetry
  - Known gaps: incomplete live MCP tool execution and tool/callback observability in current probe set
  - Class: `control_capable`

- **Auggie CLI ACP**
  - Strong: authenticated execution, `session/new`, `session/load`, permission callbacks, edit settlement
  - Known gaps: weaker persisted mutation truth for mode/model
  - Class: `control_capable`

- **Junie CLI ACP**
  - Strong: authenticated execution, thought/message streaming, permission and MCP tool callbacks
  - Known gaps: incomplete replay truth and persisted mode/model mutation truth
  - Class: `control_capable`

All three are added as second-wave, capability-gated providers rather than operator-grade default runtime targets.

### 4.3 Rollout order

Use staged feature flags:

1. ship `runtime_profile` + backend profile scaffolding and preflight compatibility checks
2. ship `codex_acp` adapter and run proof gate first
3. ship `auggie_cli_acp` and `junie_cli_acp` adapters in one follow-up feature group

Rollout may be stopped if:

- canonical run/report/recovery truth degrades
- probe evidence shows a provider no longer satisfies basic load/update/prompt requirements
- provider-specific proof shows destructive MCP resolution mismatch compared with the current runtime contract

### 4.4 Preflight and policy interaction

For each provider:

- preflight validates requested MCP/agent runtime intent against adapter capability declarations
- required MCP and optional MCP are preserved in separate requested/predicted/actual layers
- successful capability checks do not imply operator-grade classification

This keeps the proposal faithful to the current core invariant: transport-neutral Forge truth above vendor runtime details.

### 4.5 Catalog configuration direction

Expected model for this proposal:

```yaml
runtime_profiles:
  codex_acp:
    kind: acp
    adapter_family: codex_acp
    capability_class: control_capable
    requires:
      - session_new
      - session_load
      - session_update
      - permission_callbacks
      - mcp_attach

  auggie_cli_acp:
    kind: acp
    adapter_family: auggie_cli_acp
    capability_class: control_capable
    requires:
      - session_new
      - session_load
      - session_update
      - permission_callbacks
      - tool_call_visibility
      - mcp_attach

  junie_cli_acp:
    kind: acp
    adapter_family: junie_cli_acp
    capability_class: control_capable
    requires:
      - session_new
      - session_update
      - permission_callbacks
      - tool_call_visibility
      - mcp_attach

backend_profiles:
  codex_orchestrator_acp:
    provider: openai
    model: gpt-5
    effort: medium
    runtime_profile: codex_acp

  auggie_orchestrator_acp:
    provider: auggie
    model: auggie-default
    effort: medium
    runtime_profile: auggie_cli_acp

  junie_orchestrator_acp:
    provider: junie
    model: junie-default
    effort: medium
    runtime_profile: junie_cli_acp
```

The example names are placeholders; final IDs remain implementation-driven but follow the existing `*_acp` profile naming convention established in the current runtime baseline.

---

## 5. Acceptance Criteria

Proposal 029 is complete when:

1. Three new second-wave runtime profiles exist and are selected via backend profiles.
2. No new transport semantics leak into orchestration/core beyond adapter boundaries.
3. The default Goose path remains operational under the current baseline behavior.
4. Capability classes are respected in preflight so the UI/operator surface cannot assume operator-grade behavior for these profiles unless evidence grows.
5. Run snapshots and execution reports preserve persisted truth consistently across default/second-wave profiles.
6. Preflight blocks unsafe MCP contracts based on provider-specific capability declarations.
7. Proposal documentation explicitly records remaining evidence gaps per provider (tool callbacks, replay durability, and persisted mutation truth).
8. Once these three providers ship and complete same-tree proof, the implementation enters transport simplification phase in [Proposal 030](030-remove-goose-from-canonical-transport-and-simplify-runtime.md).

---

## 6. Risks

- capability regression: one provider’s behavior changes after onboarding and weakens actual proof quality
- capability inflation: treating these providers as operator-grade before `set_mode`, `set_model`, and MCP execution are consistently durable
- onboarding debt: local/runtime-specific auth/config steps may increase operator onboarding burden if not documented

---

## 7. Alternatives Considered

### 7.1 Add only one provider

Reject. Three providers are supported by one shared migration pattern and avoid repeated doc/proof overhead.

### 7.2 Add Codex only

Reject. It improves capability breadth but does not validate the broader second-wave class behavior needed for runtime-profile scale-up.

### 7.3 Defer Junie due replay weakness

Reject. Junie remains a strong control-capable runtime candidate and provides value for MCP and tool-call visibility; its known gaps are suitable for `control_capable` rollout.
