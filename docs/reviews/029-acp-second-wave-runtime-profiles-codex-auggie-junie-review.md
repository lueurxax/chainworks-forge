# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/per-agent-mcp-policy-and-runtime-validation.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-remove-goose-from-canonical-transport-and-simplify-runtime.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
- Baseline reused:
  - repo-level baseline shape
  - stable reference docs for ACP transport, provider platform, MCP truth, and execution truth
- Baseline refreshed:
  - targeted code refresh for transport factory, MCP policy runtime, runtime bridge, provider family/settings surfaces, provider selection, and current gate registry
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - none present
- Targeted context refresh performed:
  - yes, repo-local only
- External research used: `None`
- Research pack:
  - none
- Sources reused:
  - stable reference docs and current baseline
- Sources refreshed:
  - current code paths only
- Time-sensitive external guidance:
  - none
- Code areas inspected:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift`
  - `Chainworks Forge/Providers/ProviderRegistry.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `scripts/test-gate.sh`
- Current repo contradictions found:
  - the old provider-identity drift is closed
  - the old fail-closed factory owner gap is closed
  - the old Goose-owned MCP registry gap is closed
  - the old capability-owner split is closed
  - the old rollout-owner ambiguity is closed by explicit disabled-provider semantics
- Runtime evidence used: `None`
- Provenance of key evidence:
  - local proposal/docs + current code inspection + reusable baseline
- Remaining assumptions:
  - no hidden rollout-flag system exists outside checked-in repo
- Remaining blockers:
  - none

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top strengths:
  1. `P029` now names the real owner chain for the fail-closed factory change rather than describing a local patch in isolation.
  2. MCP migration now targets the actual Goose-owned registry seam instead of pretending namespace strings were the remaining problem.
  3. Rollout gating is now explicitly tied to `ConfiguredProvider.isEnabled`, family-level selection, seeded defaults, preflight, settings UI, preferred-provider repair, and diagnostics.
- Residual implementation risks:
  1. This review is proposal-text only; the repo still needs the actual `proposal-029` implementation and focused proof gate.
  2. Second-wave providers remain intentionally non-operator-grade until implementation evidence exists.
- Readiness call:
  - The draft is now implementation-ready on local proposal/doc/code/baseline evidence.

## 2. Proposal Scope and Completeness
- In scope:
  - second-wave ACP runtime onboarding
  - provider-platform expansion
  - fail-closed transport factory
  - transport-neutral MCP registry ownership
  - capability enforcement through existing provider capability truth
  - disabled-provider rollout semantics
  - focused `proposal-029` verification
- Out of scope:
  - Goose removal as canonical transport
  - hard cutover
  - operator-grade classification
  - MCP parity claims
- Deferred intentionally:
  - Goose simplification in `P030`
- Most important baseline refreshes performed:
  - current transport factory contract
  - current MCP resolution owner path
  - current provider settings/provider family model
  - current preferred-provider selection path
  - current test-gate coverage
- Most important contradictions with current repo:
  - none remain at proposal-text level
- Most important missing or partial states:
  - no blocking proposal-text gap remains

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live `proposal-text` UI findings.

### 5.2 UX Findings
- No live `proposal-text` UX findings.

### 5.3 iOS Architecture Findings
- No live `proposal-text` architecture findings.

## 6. Cross-Discipline Conflicts and Decisions
- Decision: keep provider family as the operator-facing brand axis, keep transport separate, and keep rollout truth machine-local on `ConfiguredProvider.isEnabled`.
- Decision: keep disabled second-wave providers visible in Settings but unavailable to resolution/preflight until enabled.
- Decision: keep MCP/report/recovery truth extending current persisted Forge owners instead of inventing parallel lanes.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Implement the proposal exactly as written and add the focused `proposal-029` gate | iOS Architecture | Implementation owner | During implementation | current ACP/runtime/provider platform | same-tree focused gate proves second-wave onboarding without reopening Goose fallback | none |
| P2 | Preserve current provider family vs transport separation in Settings and diagnostics | UI | Implementation owner | During implementation | current `ProviderSettingsView` model | operators can understand enabled, disabled, and active provider states without transport/family leakage | none |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Second-wave onboarding | fail-closed selection, transport-neutral MCP readiness, and enablement truth continuity | focused `proposal-029` gate; explicit coverage for disabled-provider selection, preflight, and repair behavior | no Goose fallback for unknown family; no disabled provider selected as effective binding; no parallel capability or enablement lanes | implementation audit after code lands | hold if runtime selection can still bypass `isEnabled` or if registry/capability truth splits from current owners |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The current local proposal/doc/code/baseline evidence is sufficient for a readiness call.

### Open Questions
- QUESTION-01: No blocking open question remains for implementation readiness. Any remaining detail is implementation sequencing, not proposal ownership.
