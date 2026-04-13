# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/run-control.md`
  - prior `038` review/evidence artifacts as stale-basis comparators
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/run-control.md`
- Baseline refreshed:
  - targeted reread of `§8.1`, `§10`, `§12.1`, `§13`, and `§14` in `P038`
  - targeted code refresh for current artifact persistence and hierarchy owners
  - targeted code refresh for report / comparison / recovery readers
  - targeted refresh for current proposal/session compaction truth
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- Targeted context refresh performed:
  - `Artifact`
  - `Run`
  - `RunArtifactHierarchy`
  - `RunArtifactHierarchyBuilder`
  - `RunReportBuilder`
  - `RunReportView`
  - `RunComparisonView`
  - `RecoverySheet`
  - `BlockedRunRecoveryView`
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/RunArtifactHierarchy.swift`
  - `Chainworks Forge/Engine/RunArtifactHierarchyBuilder.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Current repo contradictions found:
  - prior live blockers around persistence ownership, rollback authority, and shell-owned reader binding are now closed in the proposal text
  - one wording-level contradiction remains: `§12.1` correctly says `run_compaction_snapshot` is an input to current shell-owned readers, but `§10 Phase 6` and `§13.2` still talk as if separate compact-snapshot readers / a canonical snapshot surface exist
- Remaining blockers:
  - cross-section reader-authority wording still needs alignment between `§10`, `§12.1`, and `§13.2`

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong; most prior blockers are closed`
- Top residual implementation risks:
  1. `§10` and `§13.2` still partially reopen a parallel snapshot-first reader lane that `§12.1` just closed.
  2. The proposal now allows one explicit durable owner for archive/tombstone state, but implementation still has to choose and enforce that owner consistently.
  3. Current repo already uses compaction vocabulary for bounded proposal/session output reduction, so implementation still needs to keep the run-wide maintenance lane clearly separated.
- Top opportunities:
  1. Align the verification and risk sections fully with the new shell-owned reader contract.
  2. Preserve the new rollback-based failure authority and single-owner split exactly as written in `§8.1` and `§10`.
  3. Keep `run_compaction_snapshot` as a data artifact feeding current readers, not a new top-level reader surface.

## 2. Proposal Scope and Completeness
- In scope:
  - one server-owned `Compact Run` command
  - archive-eligible / duplicate / broken-link classification
  - projection rebuild
  - `run_compaction_plan`, `run_compaction_report`, `run_compaction_snapshot`
  - optional semantic summary
  - GraphQL mutation and MCP tool
  - UI changes for compacted runs
- Out of scope:
  - running-run compaction
  - workflow-state mutation
  - stage-outcome changes
  - deleting immutable reports
  - deleting recovery-critical evidence
  - deleting active session-lineage truth
  - history rewriting
- Deferred intentionally:
  - running-run partial compaction
- Most important baseline refreshes performed:
  - current artifact persistence schema
  - current hierarchy-builder / browsing projection ownership
  - current report/comparison/recovery reader ownership
  - current proposal/session compaction owner path
- Most important contradictions with current repo:
  - `§10 Phase 6` still verifies generic `compact snapshot readers` even though `§12.1` now says the snapshot is only an input to current readers
  - `§13.2` still says `compact snapshot as canonical post-compaction surface`, which partially undoes the new reader-binding text
- Most important missing or partial states:
  - full wording alignment across verification, UI binding, and risk sections

## 3. Proposal Readiness Verdict
- `Readiness = Amber`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is **not** an Evidence Gap Review. Local proposal/docs/code/baseline evidence is sufficient for a proposal-first verdict.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 0 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding. The draft no longer conflicts with the current segmented-shell structure.

### 5.2 UX Findings
- No live UX `proposal-text` finding. `§12.1` now correctly binds compaction to the current shell-owned readers.

### 5.3 iOS Architecture Findings

#### ARCH-001 - Verification and risk text still partially reopen a snapshot-first reader lane
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `BASE-03`, `BASE-05`, `BASE-06`, `MAP-05`, `MAP-06`, `MAP-07`, `MAP-08`, `MAP-09`, `REAL-01`
- Proposal refs:
  - `docs/proposals/038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md:374-398`
  - `docs/proposals/038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md:480-487`
- Current repo refs:
  - `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/operator-experience.md`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Why it matters:
  - `§12.1` now correctly says compaction must not introduce a parallel snapshot-first reader surface and that `run_compaction_snapshot` / `run_compaction_report` are inputs to current shell-owned readers. But `§10 Phase 6` still verifies generic `compact snapshot readers`, and `§13.2` still calls the snapshot the canonical post-compaction surface. That wording can still be implemented as a parallel reader lane instead of an input to the existing shell.
- Required fix:
  - Align `§10 Phase 6` and `§13.2` to the `§12.1` owner model:
    - verify the current shell-owned readers directly,
    - keep `run_compaction_snapshot` as supporting data for those readers,
    - remove any wording that makes the snapshot itself sound like a standalone post-compaction surface.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  - `§12.1` says the snapshot is an input to existing shell readers, while `§10` and `§13.2` still partially talk as if a dedicated compact-snapshot reading surface exists.
  - Decision needed: keep the current shell-reader model consistently across the whole draft.
- Conflict:
  - The proposal uses "compaction" for run-wide maintenance while current repo already uses compaction for bounded proposal/session output reduction.
  - Decision needed: keep the current explicit naming split (`run_compaction_*` vs output compaction metadata) strong during implementation.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Align `§10 Phase 6` and `§13.2` to the `§12.1` shell-owned reader contract | iOS Architecture | proposal author | Before next review | current run-surface IA baseline | proposal no longer mentions separate compact-snapshot readers/surfaces | `ARCH-001` |
| P2 | Preserve one explicit archive/tombstone owner during implementation planning | iOS Architecture | proposal author | Before handoff | current artifact persistence baseline | implementation notes choose one owner and keep compaction artifacts secondary | watchpoint from `ARCH-001` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Reader authority | whether post-compaction inspection stays inside current shell-owned readers | `§10`, `§12.1`, and `§13.2` all name the same reader owners | no separate snapshot-first diagnostics/read path | next proposal review | hold if `compact snapshot readers` or `canonical post-compaction surface` wording remains |
| Failure semantics | whether rollback keeps canonical owners authoritative after failed compaction | proposal retains rollback-first failure semantics | no competing truth between compaction artifacts and canonical readers | next proposal review | hold if failed verification reopens partial-apply reader ambiguity |
| Persistence split | whether archive/tombstone truth stays single-owned | implementation notes choose one owner store | compaction artifacts remain explanatory rather than authoritative | implementation planning checkpoint | hold if multiple storage owners are introduced |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: Should run-wide maintenance compaction reuse the word `compaction`, or should implementation add an explicit terminology note that distinguishes it from current proposal/session output compaction?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
