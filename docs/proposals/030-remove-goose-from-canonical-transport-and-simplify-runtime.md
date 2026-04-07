# Proposal 030: Remove Goose From Canonical Transport And Simplify Runtime Dispatch

| Field | Value |
|---|---|
| Date | 2026-04-07 |
| Status | Draft |
| Author | Codex |
| Depends on | [026-acp-first-runtime-transport-and-goose-decoupling.md](026-acp-first-runtime-transport-and-goose-decoupling.md), [029-acp-second-wave-runtime-profiles-codex-auggie-junie.md](029-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [026-acp-runtime-plan-additive-profiles.md](026-acp-runtime-plan-additive-profiles.md), [../reference/goose-server-transport.md](../reference/goose-server-transport.md), [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/provider-platform.md](../reference/provider-platform.md) |
| Scope | Remove Goose as the canonical runtime transport implementation after second-wave ACP providers are proven, and simplify runtime dispatch to an ACP-first core with Goose retained only as explicit optional compatibility adapter. |
| Goal | Replace the current multi-implementation transport shape with a single canonical ACP execution seam and capability-gated provider adapters. |

---

## 1. Context and Motivation

Proposal 026 established ACP-shaped transport as the canonical model but kept Goose as the default runtime.
Proposal 029 introduces the next runtime expansion: Codex ACP, Auggie CLI ACP, and Junie CLI ACP.

Once those providers are implemented and proven, we should complete the migration by removing Goose
from the canonical runtime path so that:

- runtime selection is no longer coupled to Goose defaults,
- transport code in core is reduced to ACP concepts only,
- Goose lives only as an adapter compatibility entry point, not as a model for future implementation.

---

## 2. Product Questions This Proposal Must Answer

After completion, the system should answer:

1. Can `Chainworks Forge` execute runs with Goose removed from canonical transport code while preserving existing truth contracts?
2. Can transport selection remain catalog-driven through `runtime_profile` and `backend_profile` only?
3. Can default runtime behavior remain stable after removing Goose as canonical default?
4. Can adapter-specific compatibility code for Goose be isolated and removed from core orchestration/test assumptions?
5. Can existing proof lanes be preserved while replacing transport complexity with a simpler dispatch model?

---

## 3. Scope

This proposal includes:

- hard deprecation of Goose as canonical transport implementation in core orchestration
- canonical runtime factory rewrite to instantiate ACP adapter families through one unified transport pathway
- removal of Goose-only lifecycle assumptions from execution orchestration and transport abstractions
- explicit “compatibility adapter” retention for Goose where needed
- migration of any remaining runtime-profile references away from `goose_rest_sse` as default mandatory path
- updating failure evidence and proof expectations for transport simplification

This proposal does not include:

- removing Goose app support from operator workflows entirely
- deleting Goose binary launch tooling from all system-level settings
- changing provider-specific policy semantics or MCP truth model
- relaxing execution/recovery/report truth guarantees

---

## 4. Design

### 4.1 Canonical seam is ACP-only

Core runtime execution should contain only ACP-shaped abstractions:

- session lifecycle
- prompt streaming
- cancel/close
- state inspection where available
- capability discovery
- session update/mode/model mutation events

No direct Goose endpoint semantics (e.g., `/agent/start`, `/agent/update_provider`) remain in core transport code.

### 4.2 Transport dispatch simplification

Replace current mixed transport selection with:

- one canonical runtime dispatch entry (`ACPRuntimeTransport` / equivalent)
- adapter registration by `adapter_family`
- strict capability checks before launch
- provider-specific execution adapters implement only transport-facing mechanics

Expected mapping:

- `provider != claude_agent_acp|gemini_cli_acp|codex_acp|auggie_cli_acp|junie_cli_acp`: rejected by catalog/preflight
- Goose runtime family can still exist under a compatibility adapter namespace if needed for migration fallback

### 4.3 Goose as compatibility adapter, not canonical path

Goose-related code in implementation should be moved to compatibility-only modules, including:

- Goose session-specific extension reconciliation bridge
- local `goosed` bootstrapping/probe helpers
- Goose extension IDs and response normalization that are no longer needed for new provider families

These modules should:

- be clearly marked compatibility-only
- have narrower surface
- remain behind profile gating and explicit diagnostics flags

### 4.4 Runtime-profile and backend-profile migration

After this proposal:

- `runtime_profiles` for default operation should not point to Goose unless explicitly selected by operator choice
- catalog defaults should shift to one of the implemented ACP profiles as first option
- `backend_profile` remains the run-time contract anchor: provider + model + runtime profile

### 4.5 Documentation and evidence

Update evidence and proposal-index docs to mark this as “Goose transport canonical path retired.”

---

## 5. Rollout Sequence

1. Complete Proposal 029 proof lane for all second-wave providers and capture pass/fail evidence.
2. Freeze default runtime selection against non-Goose ACP profile in the catalog.
3. Refactor core transport factory and remove Goose-specific path from canonical interfaces.
4. Promote Goose adapter modules to compatibility-only packaging and guard behind explicit compatibility selection.
5. Run focused proof gates:
   - Proposal 026 canonical transport lane stability
   - MCP runtime-policy and execution truth lane
   - proposal-focused regression for new second-wave adapters
6. Decommission Goose-as-default in tests and examples that depend on it as the canonical path.

---

## 6. Acceptance Criteria

Proposal 030 is complete when:

1. Core orchestration transport has no Goose-shaped canonical assumptions.
2. Runtime dispatch is uniform and ACP-first.
3. Goose is retained only as optional compatibility adapter code, not canonical default.
4. `backend_profile` + `runtime_profile` selection works for the non-Goose default and at least one additional ACP provider.
5. Canonical execution truth (`RunStartSnapshot`, `AgentExecution`, report/recovery readers) remains unchanged in semantics and fidelity.
6. Test and proof lanes pass for the post-removal transport shape with explicit, documented fallback if Goose adapter remains enabled for compatibility.
7. Operator-facing docs and onboarding no longer imply Goose is the canonical runtime shape.

---

## 7. Risks

- Migration risk: hidden Goose assumptions remain in core code paths and cause runtime regressions.
- Capability mismatch risk: one or more ACP providers may regress and force temporary backtracking in default selection.
- Onboarding risk: if Goose is demoted, local operator workflows that still rely on Goose tooling may require clearer migration guidance.

---

## 8. Alternatives Considered

### 8.1 Keep Goose as canonical with translation adapters

Rejected. This retains avoidable coupling and delays the core simplification target.

### 8.2 Remove Goose entirely from codebase

Rejected for this phase. Compatibility support remains valuable for environments still using Goose.

### 8.3 Defer simplification until a fourth provider lands

Rejected. Once three providers are proven, transport complexity is high relative to remaining benefit; this is the right time for simplification.
