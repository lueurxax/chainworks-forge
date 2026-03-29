# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `.` |
| Git SHA | `5c870b4` |
| Reviewed At | `2026-03-29T13:38:05+0300` |
| Review Mode | `proposal-readiness` |
| Product Overlay | `omitted` |
| Overall Status | `Full Review` |
| Readiness | `Green` |
| Confidence | `High` |
| Evidence Completeness | `Complete` |

## 0. Review Mode and Proposal Evidence Summary

- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- No-delta repeat round: `no`
- Proposal / docs reviewed:
  - `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/full-mvp-delivery.md`
  - `docs/reference/mvp-sign-off.md`
  - prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md`
  - prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md`
  - proposal-local research pack: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`
- Reusable baseline used: `.review-baselines/current-system-baseline.md`
- Baseline reused: `yes`
- Baseline refreshed: `partially, via targeted declarative-coverage and retry/evidence seam refresh`
- Baseline freshness: `Fresh for repo-level context, Partial for P013-specific seams`
- Proposal-specific integration context: `none`
- Targeted context refresh performed:
  - Appendix `B` tiering and Layer `Q` boundary
  - output-contract authority and hardcoded fallback branches
  - same-stage retry storage truth
  - failure-evidence ordering
  - recovery-surface ownership and action scope
- External research used: `reused prior proposal-local research pack; no fresh browsing in this round`
- Code areas inspected:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/WorkflowDefinition.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/AgentExecutor.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/GooseTransport.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `examples/agents/agents.yaml`
  - `examples/workflows/workflow.yaml`
- Current repo contradictions found:
  - none proposal-blocking on the current working-tree draft
- Runtime evidence used: `none required`
- Remaining assumptions:
  - proposal readiness can be judged from proposal/doc/code/baseline evidence without a new runtime replay
  - Appendix `B` is intended as a runtime-truth audit with explicit tiering, not as a promise to solve every decoded YAML row in this slice
- Remaining blockers:
  - none

## 1. Executive Summary

- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Implementation-ready from local proposal/doc/code/baseline evidence`
- Top strengths:
  1. Contract authority still stays singular and derived from `AgentCatalog.contracts`.
  2. Same-stage `Retry Failed Agent` remains fully grounded in current immutable stage-attempt storage truth.
  3. Failure-evidence ordering still matches the real `AgentExecutor -> ArtifactManager -> WorkflowOrchestrator.validateStructuredOutputs(...)` seam.
  4. Recovery UX still extends the current shell-owned `RecoverySheet` and `BlockedRunRecoveryView`.
  5. The new declarative-runtime appendix is now properly tiered, which closes the earlier scope-boundary blocker.

The earlier `Amber` blocker is closed. Proposal 013 now draws a usable implementation boundary: Tier 1 is limited to `contracts.*` plus `backend_profiles.*.structured_output`, while the rest of Appendix `B` is explicitly classified as metadata-only or later-platform work. That makes Layer `Q`, verification, and acceptance criteria bounded again and keeps the proposal handoffable.

## 2. Proposal Scope and Completeness

- In scope:
  - output-contract authority and review-output contract alignment
  - failed-stage evidence preservation and canonical validation-failure reference truth
  - retry lineage truth for same-run retry versus clone-run
  - blocked-run recovery explanation and narrow-action trust
  - proposal-draft output compaction metadata
  - Tier 1 declarative-runtime hardening for `contracts.*` and `structured_output`
- Out of scope:
  - provider-family expansion
  - workflow-topology redesign
  - repo-backed delivery changes already owned by `docs/reference/full-mvp-delivery.md`
  - general UI polish already owned by Proposal 012
  - wholesale rewrite of purely descriptive YAML metadata
- Deferred intentionally:
  - feature-flag rollout
  - analytics / product KPI overlay
  - broad historical artifact migration
  - Tier 3 declarative-runtime gaps such as `skill_ref`, `required_tools`, transport-level policy enforcement, and broader workflow coverage
- Most important baseline inputs reused:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
- Most important repo alignment checks:
  - `Section 4.2` still matches the current catalog-backed `OutputContractResolver` seam
  - `Section 4.2.2` now narrows mandatory declarative work to Tier 1 only
  - `Section 5.4` still matches current stage-attempt immutability and artifact path layout
  - `Section 6.2` still matches the current executor / persist / validate boundary
  - `Section 7` still matches current shell-owned recovery surfaces
  - Appendix `B` tiering is locally truthful against current parser / compiler / transport code

## 3. External Research Summary

Prior proposal-local research was reused in this round via `013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`. No fresh external browsing was needed because the new delta was repo-local and code-checkable: the key question was whether the new tiered Appendix `B` actually closed the previous scope issue.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 1 |

## 5. Findings by Discipline

### 5.1 UI Findings

No live UI findings in this reread.

### 5.2 UX Findings

No live UX findings in this reread.

### 5.3 iOS Architecture Findings

- `ARCH-01`
  - Severity: `Low`
  - Confidence: `High`
  - Evidence IDs: `DOC-01`
  - Finding: Section `2` question `6` still speaks in terms of every execution-critical field either becoming runtime truth, failing closed, or being demoted to metadata-only, while the new Appendix `B` explicitly keeps some execution-relevant rows in Tier `3` for later proposal work.
  - Why it matters: the implementation boundary is now clear in `4.2.2` and acceptance `9-10`, but this one earlier question still overstates the tiered model.
  - Fix: rephrase question `6` to reference Appendix `B` tiering directly.
  - Acceptance criteria: top-level problem statement and acceptance language point at the same tiered boundary.

## 6. Cross-Discipline Conflicts and Decisions

- Conflict:
  the proposal wants to stay honest about broader declarative-runtime drift without turning itself into an unbounded cleanup bucket
- Tradeoff:
  Appendix `B` now preserves that honesty while keeping runtime enforcement limited to Tier `1`
- Decision:
  the proposal is ready to hand off as a bounded slice
- Owner:
  proposal author

## 7. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Optional: align Section `2` question `6` with the new Appendix `B` tiering language | iOS Architecture | proposal author | next edit pass | current tiered Appendix `B`, `4.2.2`, `11` | top-level framing no longer overstates Tier `3` scope | `ARCH-01` |
| P3 | Optional: add `013...review/integration-context.md` if this proposal keeps evolving across rerounds | Review process | repo maintainer | future review hygiene | current baseline + current evidence pack | later rounds can reuse the narrow declarative-coverage mapping directly | none |

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Contract alignment | review outputs, runtime validation, artifacts, and reports all agree on one contract truth | removal or explicit isolation of hardcoded output-name branches; coherent contract metadata | do not create a second contract authority | future implementation audit | hold if runtime still reads parallel or ambiguous contract truth |
| Same-stage agent retry | prior failed evidence stays inspectable while the latest successful retry becomes effective output | agent-attempt lineage fields, disjoint retry namespace, reused sibling references | do not overwrite stage-attempt-primary artifacts | future implementation audit | hold if same-stage retry collapses back to ambiguous or colliding storage |
| Tier 1 declarative coverage | mandatory YAML families become executable truth, fail-closed, or explicitly preflight-rejected | `contracts.*` no longer depend on hardcoded fallback branches; `structured_output` reaches transport or fails preflight; tier report persisted | do not quietly grow Tier `1` back into a general YAML cleanup slice | next implementation audit | hold if Tier `1` fields still silently no-op |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: no proposal-local `integration-context.md` exists yet; this is no longer blocking because baseline reuse plus targeted refresh were sufficient.

### Open Questions

- none proposal-blocking in the current reread

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code/baseline evidence was sufficient for a full proposal-readiness review, and no live proposal-blocking findings remain.
