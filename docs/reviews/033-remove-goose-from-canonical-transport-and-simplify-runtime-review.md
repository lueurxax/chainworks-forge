# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/per-agent-mcp-policy-and-runtime-validation.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/workflow-execution-engine.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for MCP dual-path behavior in `RuntimeSessionBridge`
  - targeted code refresh for ACP adapter MCP injection
  - targeted code refresh for current `mcp_server_registry` ownership and trust readers
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - `P033` now correctly fail-closes the `P030` dependency, but `P030` is still red today
  - proposal Phase 1 still narrows legacy MCP path to Goose-backed runs, while current ACP runtimes also consume that path
  - proposal Phase 3 still leaves final `mcp_server_registry` authority ambiguous
  - proposal-owned `proposal-033` gate is still not operationally specified enough

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved`
- What improved:
  1. Hard prerequisite gating is now explicit and fail-closed.
  2. MCP migration is no longer a one-step deletion.
  3. Operator-surface migration and legacy trust fallback are now explicitly addressed.
- What still blocks `Green`:
  1. The dual-path MCP migration is still mis-scoped versus current ACP runtime reality.
  2. The final owner of runtime-namespace MCP mapping is still ambiguous.
  3. The focused proof gate is still not defined concretely enough to verify the slice end-to-end.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP-first runtime dispatch
  - Goose compatibility-only packaging
  - phased MCP ownership migration
  - operator-surface migration
  - trust-vocabulary normalization with legacy fallback
  - proposal-specific proof gating
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from all system-level settings
  - weakening execution/recovery/report truth
- External hold:
  - `P030` is still red, so implementation cannot start yet even if `P033` proposal quality improves

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 2 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `MAP-01`, `MAP-02`, `MAP-03`, `REAL-01`
  Why it matters: Phase 1 is still too narrow for the current runtime. The proposal says the old `mcp_profile` / `mcp_server_registry` path continues unchanged for Goose-backed runs, but current runtime flow resolves MCP policy before the transport split and then feeds ACP adapters through `mcpServers`. That means the legacy MCP path is still part of the current ACP path too, not only Goose compatibility. As written, the proposal can still be implemented as a non-incremental break instead of a true dual-path migration.
  Recommended fix: rewrite Phase 1 so the old path remains valid for all current cataloged agents, including ACP-backed agents, until their `backend_profile` explicitly carries `mcp_intent`. Also define precedence when both `agent.mcp_profile` and `backend_profile.mcp_intent` exist.
  Acceptance criteria:
  - Phase 1 explicitly preserves legacy MCP declaration for both Goose and ACP-backed agents
  - precedence between `mcp_intent` and `mcp_profile` is explicit
  - the proposal requires proof for ACP + Goose dual-path behavior before Phase 2 deprecation starts
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-06`, `MAP-04`, `MAP-05`, `MAP-06`, `REAL-02`
  Why it matters: Phase 3 still does not lock the final authority for runtime-namespace MCP mapping. The proposal says `mcp_server_registry` stays if adapters still need runtime mapping, otherwise it moves to machine-local config. Current baseline treats that registry as repo-owned canonical truth. Without explicitly deciding the end state, `P033` can finish in two different architectures, and acceptance `8` cannot be judged deterministically.
  Recommended fix: lock one canonical end state for `mcp_server_registry` at the end of `P033`, or explicitly defer registry relocation to a later proposal and keep repo-owned registry authoritative throughout this slice.
  Acceptance criteria:
  - the post-`P033` owner of runtime-namespace MCP mapping is explicit
  - the condition for keeping versus relocating registry truth is explicit and testable
  - acceptance `8` can be evaluated without interpretive choice by implementers
  Confidence: `High`

- Finding ID: `ARCH-033-003`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-07`, `TEST-01`, `TEST-02`, `REAL-03`
  Why it matters: The proposal now owns a `proposal-033` gate, but the gate is still only conceptual. It does not name the exact same-tree suites or evidence outputs that prove the dual-path MCP migration, legacy trust fallback, and Goose compatibility behavior. For a slice that changes transport truth, MCP truth, and operator trust vocabulary together, that leaves too much room for a “green” gate with incomplete coverage.
  Recommended fix: define the concrete gate composition, including named test targets or gate groups and the expected evidence outputs for prereq, dual-path MCP, trust-reader fallback, and Goose-compatibility proof.
  Acceptance criteria:
  - `proposal-033` names concrete test suites or gate groups
  - the gate explicitly covers P030 prerequisite, ACP/Goose dual-path MCP behavior, and legacy trust-value fallback
  - the proposal names the proof artifacts that will demonstrate success
  Confidence: `Medium`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants a safe dual-path MCP migration, but its current wording still limits the old path to Goose-backed runs while current ACP execution still depends on that path.
  Tradeoff: narrowing the old path would simplify the story, but it would make the migration non-incremental and inconsistent with current runtime behavior.
  Decision: keep the old path alive for both Goose and ACP until explicit backend-profile migration is proven.
  Owner: proposal author

- Conflict: the proposal wants to simplify canonical runtime truth, but its Phase 3 MCP registry owner is still conditional rather than fixed.
  Tradeoff: leaving the choice open preserves flexibility, but it weakens architectural closure and acceptance testing.
  Decision: lock the end-state owner now or defer registry relocation out of this proposal.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Rewrite Phase 1 MCP dual-path so legacy `mcp_profile` remains valid for ACP-backed agents too, with explicit precedence over/under `mcp_intent` | iOS Architecture | Proposal author | Before implementation | current ACP runtime behavior | no implementer has to guess whether legacy MCP survives for ACP agents | `ARCH-033-001` |
| P0 | Lock the final authority for `mcp_server_registry` at the end of `P033`, or explicitly defer relocation to a later slice | iOS Architecture | Proposal author | Before implementation | current repo-owned MCP truth | Phase 3 has one deterministic end state | `ARCH-033-002` |
| P1 | Define exact `proposal-033` gate composition and proof outputs | iOS Architecture | Proposal author | Before implementation | P030 prerequisite semantics | focused proof lane is operationally testable, not just conceptually named | `ARCH-033-003` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Dual-path MCP migration | ACP and Goose both survive Phase 1 without truth loss | explicit precedence and dual-path proof | no current ACP flow is broken by early agent-level removal | next rereview of `P033` | hold if Phase 1 still reads Goose-only |
| MCP registry authority | end-state ownership is deterministic | explicit final owner and phase boundary | no split repo/local truth is left implicit | next rereview of `P033` | hold if Phase 3 can still end in two architectures |
| Focused proof lane | proposal-specific verification is concrete | named suites and evidence outputs | no acceptance criterion depends on unspecified proof | next rereview of `P033` | hold if `proposal-033` remains conceptual only |
| External dependency | `P030` readiness | all-family ACP proof green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The review has enough local proposal/docs/code/baseline evidence; the remaining issues are proposal-text and dependency-state issues.

### Open Questions
- QUESTION-01: when both `agent.mcp_profile` and `backend_profile.mcp_intent` are present, which one wins during Phase 1?
- QUESTION-02: does `P033` intend to keep `mcp_server_registry` repo-owned through the whole slice, or is registry relocation a separate future change?
- QUESTION-03: what exact same-tree gate composition proves this proposal complete?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
