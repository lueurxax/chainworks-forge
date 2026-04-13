# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/038-run-compaction-artifact-governance-and-canonical-snapshot-maintenance.md` | 2026-04-12 | High | Current draft now includes explicit canonical persistence owners in `§8.1`, rollback-based failure authority in `§10`, and shell-owned reader binding in `§12.1`. | Review could preserve stale blockers if it uses the older red basis. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-12 | High | Intake baseline points proposal review toward stable reference docs rather than proposal archaeology. | Review could skip the stable baseline chain. | Intake baseline. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-12 | High | Stable current-system baseline already assumes the segmented operator shell, immutable reports, run comparison, recovery, artifact hierarchy, and ACP-era runtime. | Proposal must integrate with current shell/runtime reality. | Stable baseline anchor. |
| DOC-04 | `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md` | 2026-04-12 | High | `RunArtifactHierarchy` is canonical for browsing only, while report/comparison/recovery/export authority stays on existing shell-owned readers. | Compaction wording must not reopen a second reader lane. | Primary shell-owner reference. |
| DOC-05 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-12 | High | Current repo already has bounded proposal/session output compaction truth and preserves compaction metadata/outcome evidence there. | Run-wide compaction must stay distinct from current output-compaction semantics. | Current compaction baseline. |
| DOC-06 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-12 | High | Report/recovery readers already prefer persisted execution/stage truth over heuristic reconstruction. | Failed compaction must keep those readers authoritative. | Recovery/report precedence reference. |
| DOC-07 | `docs/reference/operator-experience.md` | 2026-04-12 | High | Current operator shell already owns reports, comparison, artifact inspection, and recovery inside one run-detail spine. | Generic snapshot-first wording can still fork from this baseline. | Operator shell baseline. |
| DOC-08 | `docs/reference/run-control.md` | 2026-04-12 | High | Current product already distinguishes execution control from archive visibility and treats archive as its own operator truth. | Artifact/run-compaction archive semantics still need consistent ownership. | Archive semantics baseline. |
| DOC-09 | prior `038` review / evidence artifacts | 2026-04-12 | High | Prior review basis is now stale for persistence-owner, rollback, and shell-reader-binding blockers. | Review could re-emit already-closed findings. | Freshness comparator. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake routing | 2026-04-12 | High | Fresh enough as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current product/runtime shell baseline | 2026-04-12 | High | Fresh and directly relevant. | Stable baseline authority. |
| BASE-03 | `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md` | Reused | artifact browsing / shell-owned readers / pane ownership | 2026-04-12 | High | Fresh and central to this slice. | Main shell-owner authority. |
| BASE-04 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | Reused | failure evidence + current output-compaction truth | 2026-04-12 | High | Fresh and directly relevant. | Compaction-owner baseline. |
| BASE-05 | `docs/reference/execution-truth-and-recovery.md` | Reused | report/recovery read order | 2026-04-12 | High | Fresh and directly relevant. | Recovery/report authority baseline. |
| BASE-06 | `docs/reference/operator-experience.md` | Reused | reports/comparison/artifact inspection/recovery shell | 2026-04-12 | High | Fresh and directly relevant. | Operator-shell baseline. |
| BASE-07 | `docs/reference/run-control.md` | Reused | stop/archive semantic separation | 2026-04-12 | High | Fresh and directly relevant. | Archive vocabulary baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - one server-owned `Compact Run` command
  - artifact classification, archive eligibility, exact duplicate detection, link repair, projection rebuild
  - `run_compaction_plan`, `run_compaction_report`, `run_compaction_snapshot`
  - optional semantic summary/clustering
  - GraphQL mutation and MCP tool
  - compacted active run surface
- Out of scope:
  - compaction for running runs
  - workflow-state mutation
  - changing stage outcomes
  - deleting immutable reports
  - deleting recovery-critical evidence
  - deleting active session-lineage truth
  - rewriting run history
  - model-owned destructive maintenance
- Deferred intentionally:
  - running-run partial compaction
- Assumptions:
  - review mode is `proposal-readiness`
  - local proposal/docs/code evidence is sufficient
- Open questions:
  - whether implementation should carry an explicit terminology note that distinguishes run-wide maintenance compaction from current proposal/session output compaction
- Blockers:
  - `§10` / `§13.2` still partially conflict with the new `§12.1` shell-owned reader model

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RunsHomeView` / `Idea` segmented run surfaces | Baseline | 2026-04-12 | High | Current run shell already routes operators into existing summary/progress/flow/artifact/recovery panes; there is no separate compaction-first screen today. | Proposal could accidentally define a parallel run surface. | Shell entry point. |
| NAV-02 | `RunArtifactHierarchyView` artifact browser | Baseline + current repo | 2026-04-12 | High | Current artifact hierarchy is the shared browsing owner and is consumed from both run surfaces. | Proposal can fork inspection if it invents a snapshot-first browser. | Artifact navigation owner. |
| NAV-03 | `RunReportView` / `RunComparisonView` / `RecoverySheet` / `BlockedRunRecoveryView` | Baseline + current repo | 2026-04-12 | High | Current operator spine already owns report history, comparison, and recovery. | Proposal can fork the operator spine if it adds separate compact-snapshot readers. | Reader ownership. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/Artifact.swift` | persistence | canonical persisted artifact identity and lineage | 2026-04-12 | High | Current artifact schema still has no archive/tombstone/compaction-class fields, but the proposal now explicitly constrains that future truth to one canonical owner. | Implementation can still drift if it ignores the single-owner requirement. | Persistence seam. |
| MAP-02 | `Chainworks Forge/Models/RunArtifactHierarchy.swift` | projection | shared browsing projection | 2026-04-12 | High | Hierarchy already carries promoted/latest-summary/latest-report browsing metadata and is explicitly not a second truth lane. | Proposal wording must keep the hierarchy as a reader, not a competing truth source. | Main browsing seam. |
| MAP-03 | `Chainworks Forge/Engine/RunArtifactHierarchyBuilder.swift` | projection build | centralized artifact grouping | 2026-04-12 | High | Current builder groups artifacts for browsing from persisted truth; it does not own archive/compaction settlement. | Proposal wording must not move authority into the builder path. | Hierarchy-owner seam. |
| MAP-04 | `Chainworks Forge/Models/Run.swift` | persistence | run-level latest summary/report pointers and promoted artifacts | 2026-04-12 | High | Current run schema already owns latest-summary/latest-immutable-report pointers and promoted artifact names. | Proposal now aligns with this owner model, but implementation still needs to choose the exact archive/tombstone store. | Run metadata seam. |
| MAP-05 | `Chainworks Forge/Engine/RunReportBuilder.swift` | report generation | current canonical report selection and recovery summary | 2026-04-12 | High | Report builder already prefers canonical lineage, pinned artifacts, and current recovery truth. | `§10` / `§13.2` wording can still imply a competing reader lane. | Report-owner seam. |
| MAP-06 | `Chainworks Forge/Views/RunReportView.swift` | shell reader | immutable history / latest summary / export hub | 2026-04-12 | High | Current shell already has a named run-report reader. | Snapshot-first wording can still fork report ownership. | Reader continuity. |
| MAP-07 | `Chainworks Forge/Views/RunComparisonView.swift` | shell reader | deterministic run comparison | 2026-04-12 | High | Current comparison view is already a named owner path. | Proposal needs to keep compaction subordinate to it. | Comparison continuity. |
| MAP-08 | `Chainworks Forge/Views/RecoverySheet.swift` | shell reader | narrow blocked/failed recovery reader | 2026-04-12 | High | Current recovery reader already loads persisted receipts/stage history/evidence packets. | Failed compaction must still leave this reader authoritative. | Recovery continuity. |
| MAP-09 | `Chainworks Forge/Views/BlockedRunRecoveryView.swift` | shell reader | blocked run re-entry surface | 2026-04-12 | High | Current blocked-run recovery is explicitly shell-owned and not a parallel destination. | Proposal wording must not route compaction failures into a second diagnostics lane. | Recovery continuity. |
| MAP-10 | `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift` + `Chainworks Forge/Models/AgentExecution.swift` | current compaction owner | bounded proposal/session output compaction | 2026-04-12 | High | Current repo already uses compaction vocabulary and stores `compactionMetadataJSON` on `AgentExecution`. | Run-wide compaction remains a semantic watchpoint, though no longer a live blocker. | Compaction vocabulary seam. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | artifact persistence schema | `Artifact.swift` + `P038 §8.1` | persisted artifact truth -> readers | 2026-04-12 | High | Proposal now explicitly separates durable artifact truth from compaction-only classifications and requires one explicit archive/tombstone owner. | Remaining risk is implementation drift, not proposal ambiguity. | Previously-blocking architecture seam. |
| DATA-02 | run-level latest report / summary pointers | `Run.swift` + `P038 §8.1` | persisted run truth -> report/hierarchy | 2026-04-12 | High | Proposal now keeps latest-summary/latest-report truth on canonical run owners. | Main persistence ambiguity is now closed at proposal level. | Snapshot-owner seam. |
| DATA-03 | report/recovery read order | `RunReportBuilder.swift`, `RunReportView.swift`, `RecoverySheet.swift`, `BlockedRunRecoveryView.swift`, `execution-truth-and-recovery.md`, `P038 §10` | persisted truth -> shell readers | 2026-04-12 | High | Proposal now explicitly rolls back failed compaction and says readers follow canonical owners. | Only wording alignment around snapshot readers remains. | Failure-authority seam. |
| DATA-04 | current compaction metadata | `ProposalDraftCompactionPolicy.swift`, `AgentExecution.compactionMetadataJSON`, `output-contracts-failure-evidence-and-recovery.md` | current compaction owner -> evidence/report/export | 2026-04-12 | High | Compaction already exists in current repo as a stage/session evidence concept. | Implementation still needs to keep the run-wide maintenance lane distinct. | Important watchpoint. |
| DATA-05 | northbound exposure | `P038 §11` | server -> GraphQL/MCP | 2026-04-12 | High | The draft still makes GraphQL/MCP part of its contract. | Those APIs inherit the remaining reader-authority wording issue. | API consequence. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | artifact browsing owner | Baseline + current repo | 2026-04-12 | High | `RunArtifactHierarchy` is the shared browsing projection and is not a second truth lane. | `§10` / `§13.2` still partially risk reopening a second reader lane. | Main shell seam. |
| INT-02 | report / comparison / recovery readers | Baseline + current repo | 2026-04-12 | High | Current shell already has named readers for run report, run comparison, and recovery. | `compact snapshot readers` wording is still weaker than the new `§12.1` owner model. | Main reader seam. |
| INT-03 | current compaction vocabulary | Baseline + current repo | 2026-04-12 | High | The repo already records compaction as bounded proposal/session output behavior. | Implementation should keep explicit naming separation, but the proposal no longer depends on a false owner merge. | Semantics watchpoint. |
| INT-04 | archive semantics | Baseline | 2026-04-12 | High | Current product already treats archive as a distinct visibility-control truth. | Proposal now mostly aligns; remaining risk is implementation discipline. | Archive seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01 | run surface + command eligibility | Entry conditions are clear. |
| Happy path | Specified | DOC-01, DATA-01, DATA-02, NAV-03 | server-owned command + current readers | Happy path owner chain is now explicit. |
| Loading | Deferred intentionally | DOC-01 | not a primary slice concern | Acceptable for proposal-readiness. |
| Empty | Partial | DOC-01, NAV-02 | compacted hierarchy / archive secondary view | Archive-empty and fully-compacted secondary states are still implied rather than detailed. |
| Validation error | Specified | DOC-01, DATA-01 | manual review / broken-link candidate / plan/report | Classification ownership is now explicit enough. |
| Backend error | Partial | DOC-01, DATA-03, REAL-01 | verification phase + shell readers | Failure authority is now explicit, but wording still needs alignment. |
| Offline / degraded | Deferred intentionally | DOC-01 | out of scope | Acceptable. |
| Retry / recovery | Specified | DOC-01, DATA-03, MAP-08, MAP-09 | recovery readers after compaction | Recovery continuity is now explicitly preserved. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | out of scope | Acceptable. |
| Rollback / cancellation | Specified | DOC-01, DATA-03 | deterministic apply then rollback on failed verification | Rollback semantics are now explicit. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none specified | run compaction rollout | no staged rollout or flag owner is named | proposal now explicitly prefers rollback after failed verification | 2026-04-12 | Medium | Not a blocker for proposal-readiness. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | before/after counts in `run_compaction_report` | explain compaction result | `P038 §9.2` | 2026-04-12 | High | Useful and aligned; no live blocker here. |
| METRIC-02 | verification phase reader checks | prevent broken reader surfaces | `P038 §10 Phase 6` | 2026-04-12 | High | Verification remains valuable, but its reader naming still needs alignment to `§12.1`. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | current repo | artifact hierarchy + report supersedence | existing hierarchy-builder and run-report tests already cover promoted/latest/superseded truth | proposal must add compaction-specific proof without breaking current readers | 2026-04-12 | High | Current substrate is already opinionated; proposal cannot assume a blank slate. |
| TEST-02 | proposal text | verification phase | proposal now requires rollback if canonical readers fail after apply | next pass should align reader names in verification with current shell owners | 2026-04-12 | High | Remaining wording gap only. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | reader authority after compaction | `§12.1` says snapshot/report are inputs to current shell-owned readers | `§10 Phase 6` still verifies generic `compact snapshot readers`, and `§13.2` still says `compact snapshot as canonical post-compaction surface` | 2026-04-12 | High | One live wording contradiction remains. |
| REAL-02 | compaction semantics | proposal frames compaction as a run-wide maintenance concept with named `run_compaction_*` artifacts | current repo already has stage/session compaction truth and evidence vocabulary | 2026-04-12 | High | This is now a watchpoint, not a live readiness blocker. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Problem framing is strong. |
| Scope boundaries | Specified | DOC-01 | Scope and out-of-scope are clear. |
| Reusable baseline coverage | Specified | BASE-03, BASE-04, BASE-05, BASE-06, BASE-07 | Proposal is now much better aligned with current baselines. |
| Screen / surface definition | Partial | NAV-01, NAV-02, NAV-03, REAL-01 | Current shell owners are now named, but `§10` / `§13.2` still lag behind. |
| Navigation / entry points | Partial | NAV-01, NAV-02, NAV-03 | Post-compaction path is mostly clear; the remaining issue is cross-section wording consistency. |
| State handling | Partial | H matrix | Backend-error / verification wording still needs alignment. |
| Data / API contract | Specified | DATA-01, DATA-02, DATA-05 | Core owner split is now explicit enough. |
| Persistence / caching | Specified | DATA-01, DATA-02 | Main persistence blocker is closed at proposal level. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Acceptable. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Rollback is explicit, staged rollout is not. |
| Analytics / instrumentation | Specified | METRIC-01, METRIC-02 | Sufficient for proposal-readiness. |
| Testing strategy | Partial | TEST-01, TEST-02 | Proof intent is strong; reader naming still needs alignment. |
| Dependencies / integration points | Partial | INT-01, INT-02, INT-03, INT-04 | One remaining cross-section wording gap persists. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P038` is intended to extend the current operator shell and artifact reader chain rather than replace it.
- ASSUMP-02: run-wide maintenance compaction remains distinct from the already-landed proposal/session output-compaction substrate.
- QUESTION-01: Should implementation add an explicit terminology note that distinguishes run-wide compaction from current output-compaction behavior?
- BLOCKER-01: `§10` and `§13.2` still partially contradict the new `§12.1` shell-owned reader model.

## O. Research Triggers / External Questions
No external research trigger was required for this pass. Local proposal/docs/code/baseline evidence was sufficient.
