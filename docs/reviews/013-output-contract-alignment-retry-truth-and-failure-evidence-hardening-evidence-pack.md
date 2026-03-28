# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` | 2026-03-28 | High | Current draft now explicitly derives contract truth from `AgentCatalog.contracts`, extends existing recovery owners, anchors failure-evidence ordering to the current runtime seam, defines same-stage agent-retry storage truth in Section `5.4`, makes failed-stage evidence the canonical reference target for recovery/report/export, and defaults contract mismatch to non-auto-retryable recovery posture. | Review could carry stale earlier blockers forward after the text changed. | Primary document under review. |
| DOC-02 | `docs/reference/runtime-contract.md` | 2026-03-28 | High | Runtime contract still treats stage attempts and their artifacts as immutable attempt-scoped truth. | Could misjudge whether the new Section `5.4` actually closes the prior storage-identity gap. | Anchors current runtime truth. |
| DOC-03 | `docs/reference/workflow-execution-engine.md` | 2026-03-28 | High | Artifact persistence still uses `ArtifactManager.persistOutputs(...)` plus `ArtifactStorage` path layout `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}`. | Could miss whether the proposal now correctly extends that seam. | Anchors current persistence boundary. |
| DOC-04 | `docs/reference/operator-experience.md` | 2026-03-28 | High | Current operator experience already includes shell-owned recovery surfaces and retry actions. | Could reopen closed recovery-ownership findings incorrectly. | Confirms that Section `7` is now aligned. |
| DOC-05 | `docs/reference/domain-model.md` | 2026-03-28 | High | `Artifact` remains immutable per stage attempt and only exposes `attemptNumber` today. | Could miss whether the new draft now specifies the required future delta cleanly. | Supports closure of the old storage finding. |
| DOC-06 | prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md` | 2026-03-28 | High | The previous round's live blocker was the lack of an artifact identity contract for same-stage agent retry. | Could lose track of what changed across rounds. | Supports closure analysis. |
| DOC-07 | prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md` | 2026-03-28 | High | Earlier evidence already mapped current contract, recovery, and motivating-run seams. | The reread could duplicate work or misstate prior repo reality. | Supports narrowed follow-up review. |
| DOC-08 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md` | 2026-03-28 | High | Proposal-local research already recommended three concrete deltas: explicit frozen-snapshot reuse, canonical validation-failure reference truth, and non-auto-retryable default posture for contract mismatch. | Could fail to recognize that the latest proposal revision intentionally absorbed those deltas. | Supports closure analysis for the current round. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Missing | repo-level host-system baseline | 2026-03-28 | High | No reusable baseline artifact exists in the current repo. | Direct code/doc mapping was still required. |
| BASE-02 | `<proposal>.review/integration-context.md` | Missing | proposal-local context slice | 2026-03-28 | High | No P013 integration-context artifact exists yet. | Current narrowed refresh lives only in this evidence pack. |
| BASE-03 | prior P013 review/evidence artifacts | Reused | proposal-local history | 2026-03-28 | High | Prior local review artifacts were reused to verify which findings were actually closed by the current draft. | Enables a clean delta review instead of a full restart. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- Delta round: proposal source changed after the last green pass and now absorbs the targeted follow-up clarifications from the proposal-local research pack.
- In scope:
  - proposal readiness for contract alignment, retry truth, failure evidence, blocked-run recovery, and proposal-output compaction
  - current repo mapping for artifact identity, retry lineage, persistence seams, and recovery ownership
  - closure check against the previous P013 review round and proposal-local research deltas
- Out of scope:
  - new build/run attempts
  - product KPI overlay
  - repo-backed delivery semantics beyond the specific failure class named here
  - fresh external research
- Deferred intentionally:
  - reusable baseline creation
  - implementation audit
  - runtime proof beyond existing repo-local run-storage evidence
- Assumptions:
  - proposal readiness can be judged from proposal/docs/code evidence plus prior local review artifacts
  - the current artifact path and metadata contract in code/docs remains authoritative for validating the proposal delta
- Open questions:
  - none that remain proposal-blocking after the latest revision
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RecoverySheet` | Targeted refresh | 2026-03-28 | High | Current recovery sheet already owns blocked reason, suggested action, all allowed actions, and stage history. | Could reopen the closed recovery-ownership finding incorrectly. | Confirms Section `7` is aligned with current shell ownership. |
| NAV-02 | `BlockedRunRecoveryView` | Targeted refresh | 2026-03-28 | High | Current blocked-run view already renders recovery path, preserved receipts, and action buttons. | Could misread the current draft as still implying a greenfield recovery surface. | Confirms Section `7` is a delta on existing owners. |
| NAV-03 | immutable run-report / artifact surfaces | Targeted refresh | 2026-03-28 | Medium | Current report and artifact surfaces still present stage-attempt truth today; the proposal now specifies how to extend that truth for same-stage agent retry. | Could miss whether the draft fully closes the prior gap. | Supports closure analysis. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/AgentExecutor.swift` / `OutputContractResolver` | runtime contract | current contract resolution and output naming | 2026-03-28 | High | Contract IDs for proposal-review agents still resolve through shared catalog-backed runtime logic. | Could mistakenly keep the old parallel-contract-authority finding alive. | Confirms that old contract-source finding is closed in the draft. |
| MAP-02 | `Chainworks Forge/Engine/ArtifactManager.swift` / `persistOutputs(...)` | persistence | current artifact metadata write seam | 2026-03-28 | High | Output persistence still keys writes and metadata by stage attempt number only in current code. | Could misjudge whether Section `5.4` is addressing the right seam. | Supports closure of the old storage finding. |
| MAP-03 | `Chainworks Forge/Engine/ArtifactStorage.swift` | persistence | current on-disk path layout | 2026-03-28 | High | Files still persist under `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}` in current code. | Could miss whether the proposal now specifies the required namespace extension. | Supports closure of the old storage finding. |
| MAP-04 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` / `validateStructuredOutputs(...)` | orchestration / validation | current structured-output validation point | 2026-03-28 | High | Structured validation still happens at the orchestrator seam named in Section `6.2`. | Could reopen the old persistence-boundary finding incorrectly. | Confirms that the old boundary-anchor finding is closed in the draft. |
| MAP-05 | `Chainworks Forge/Engine/RecoveryCoordinator.swift` | recovery policy | current retry mutation path | 2026-03-28 | High | Current recovery still mutates stage attempt truth and agent retry flags, and the proposal now explicitly layers over that reality. | Could miss whether retry lineage is still under-specified. | Confirms that old retry-lineage finding is closed in text. |
| MAP-06 | `Chainworks Forge/Engine/RunReportBuilder.swift` | reporting | current attempt/retry rendering | 2026-03-28 | High | Reports still render stage-attempt-oriented retry narrative today; the proposal now defines a deterministic “latest successful retry delta vs primary artifact” rule. | Could miss whether report truth remains ambiguous. | Supports closure analysis. |
| MAP-07 | `Chainworks Forge/Models/StageExecution.swift` | persistence model | stage-level attempt truth | 2026-03-28 | High | `StageExecution` still only has `attemptNumber` at stage scope. | Could overstate what the repo already supports natively. | Confirms why Section `5.4` needed to preserve stage-attempt immutability. |
| MAP-08 | `Chainworks Forge/Models/AgentExecution.swift` | persistence model | agent-level execution metadata | 2026-03-28 | High | `AgentExecution` still has `retryReason` but no current per-agent-attempt artifact identity. | Could miss whether the proposal now specifies the needed future delta. | Supports closure analysis. |
| MAP-09 | `Chainworks Forge/Models/Artifact.swift` | persistence model | durable artifact metadata | 2026-03-28 | High | `Artifact` remains immutable per stage attempt today; the proposal now explicitly defines the additional fields it needs. | Could wrongly continue to treat this as an unresolved blocker. | Confirms closure of the old storage finding. |
| MAP-10 | `examples/agents/agents.yaml` | catalog / contract truth | current output paths and contract definitions | 2026-03-28 | High | Proposal-review agents still map through one `proposal_review_v1` contract. | Could mistakenly preserve the old contract-authority finding. | Confirms that the contract-source issue is closed. |
| MAP-11 | `Chainworks Forge/Views/RecoverySheet.swift` + `BlockedRunRecoveryView.swift` | recovery presentation | current owner path for recovery UI | 2026-03-28 | High | Existing views already own the recovery surface Proposal 013 now names explicitly. | Could reopen the old net-new-recovery-surface finding incorrectly. | Confirms Section `7` alignment. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | contract truth | `examples/agents/agents.yaml` + `AgentExecutor.swift` | catalog -> runtime resolution | 2026-03-28 | High | Current contract truth is still catalog-backed and runtime-resolved. | Could fail to recognize that the old contract-authority finding is fixed. | Confirms closure of old finding. |
| DATA-02 | structured validation seam | `WorkflowOrchestrator.validateStructuredOutputs(...)` | outputs -> validation | 2026-03-28 | High | The current validation seam matches the one Section `6.2` now names explicitly. | Could fail to recognize that the old persistence-ordering finding is fixed. | Confirms closure of old finding. |
| DATA-03 | raw artifact persistence | `ArtifactManager.persistOutputs(...)` + `ArtifactStorage.write(...)` | executor -> disk/SwiftData | 2026-03-28 | High | Persistence still keys on stage attempt number and agent ID only in current code, and the proposal now explicitly extends that with `agent-retry-{agentAttemptNumber}` namespace plus lineage metadata. | Could wrongly continue to treat storage identity as unresolved. | Confirms closure of old finding. |
| DATA-04 | retry lineage persistence | `StageExecution`, `AgentExecution`, `Artifact`, `RunReportBuilder` | retry mutation -> stored evidence -> rendered history | 2026-03-28 | High | The draft now adds both agent lineage and the artifact/storage contract needed to keep same-stage retry evidence distinct. | Could miss that the remaining blocker is actually gone. | Confirms closure of old finding. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | catalog-backed output contracts | Current repo | 2026-03-28 | High | `AgentCatalog.contracts` plus `OutputContractResolver` remains the live contract-resolution seam today. | Current draft aligns with this seam. | Confirms old finding closure. |
| INT-02 | shell-owned recovery views | Current repo | 2026-03-28 | High | `RecoverySheet` and `BlockedRunRecoveryView` already own recovery presentation. | Current draft extends these owners instead of implying a parallel surface. | Confirms old finding closure. |
| INT-03 | immutable stage-attempt artifact identity | Current repo | 2026-03-28 | High | Current repo remains stage-attempt-scoped, and the draft now explicitly preserves that while adding a disjoint agent-retry namespace and lineage rule. | No remaining proposal-blocking conflict surfaced here. | Confirms closure of old finding. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `NAV-01`, `NAV-02` | blocked-run recovery and report surfaces | Entry to the reviewed slice is clear and aligned with current owners. |
| Happy path | Specified | `DOC-01`, `MAP-06`, `INT-03` | retry success without clone | Retry semantics now include the missing storage truth needed for same-stage agent retry. |
| Loading | Deferred intentionally | `DOC-01` | none material | Proposal is about persisted truth, not loading/spinner behaviour. |
| Empty | Partial | `DOC-01`, `NAV-01`, `NAV-02` | failed-stage evidence panel / recovery surface | Empty-state specificity could still be improved later, but it is not blocking implementation readiness. |
| Validation error | Specified | `DOC-01`, `DATA-02`, `DATA-03` | structured-output validation path | Validation-failure handling remains central and well anchored. |
| Backend error | Specified | `DOC-01`, `MAP-02`, `MAP-03` | execution receipt / transcript / raw outputs | Storage preservation is now spelled out for same-stage retry namespaces too. |
| Retry / recovery | Specified | `DOC-01`, `MAP-05`, `MAP-06`, `MAP-09`, `INT-03` | retry lineage and recovery/report surfaces | Action semantics, lineage, and artifact truth are now coherent. |
| Rollback / cancellation | Deferred intentionally | `DOC-01` | clone-run / run settlement only | Still outside this bounded slice. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none specified | runtime hardening slice | no rollout or flag plan described | no explicit hold/rollback note | 2026-03-28 | Medium | Still omitted, but not a proposal-blocking issue for this bounded hardening slice. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | none specified | n/a | n/a | 2026-03-28 | Medium | Product overlay remains omitted and is not needed for the current verdict. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal verification plan | contract validation, retry numbering, clone-vs-retry lineage, failed-stage evidence, proposal-drafting compaction | proposal still defines unit/integration/app/regression expectations in Sections `10` and `11` | optional future addition: make the same-stage retry namespace assertion explicit in tests | 2026-03-28 | High | Verification plan is strong enough for handoff. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | agent-only retry storage truth | proposal now defines stage-attempt immutability plus a disjoint `agent-retry-{agentAttemptNumber}` namespace and artifact lineage metadata | current repo remains stage-attempt-scoped today | 2026-03-28 | High | The draft now names the exact delta implementation needs. |
| REAL-02 | contract ownership | proposal says typed schema/resolver are derived from `AgentCatalog.contracts` | current repo already uses that seam | 2026-03-28 | High | Old contract-authority finding is closed. |
| REAL-03 | recovery ownership | proposal says it extends `RecoverySheet` and `BlockedRunRecoveryView` | current repo already uses those owners | 2026-03-28 | High | Old recovery-ownership finding is closed. |
| REAL-04 | validation/persistence ordering | proposal names `AgentExecutor -> ArtifactManager -> WorkflowOrchestrator.validateStructuredOutputs(...)` | current repo still uses that seam | 2026-03-28 | High | Old persistence-boundary finding is closed. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01` | The motivating failure class remains concrete and bounded. |
| Scope boundaries | Specified | `DOC-01` | The proposal keeps a focused scope. |
| Reusable baseline coverage | Missing | `BASE-01`, `BASE-02` | No reusable baseline or proposal-local integration context exists yet. |
| Screen / surface definition | Specified | `NAV-01`, `NAV-02`, `INT-02` | Recovery ownership matches current shell reality. |
| Navigation / entry points | Specified | `NAV-01`, `NAV-02` | No live navigation ambiguity remains in the draft. |
| State handling | Specified | state matrix above | The last major retry/evidence state gap is now closed. |
| Data / API contract | Specified | `DATA-01`, `DATA-03`, `DATA-04`, `REAL-01` | Contract authority and same-stage retry storage truth are both explicit now. |
| Persistence / caching | Specified | `MAP-02`, `MAP-03`, `MAP-09`, `REAL-01` | Failure-ordering seam and agent-retry storage identity are both explicit. |
| Permissions / auth expiry | Deferred intentionally | `DOC-01` | Outside the bounded scope. |
| Feature flags / rollout / rollback | Missing | `FLAG-01` | Still omitted, but not proposal-blocking here. |
| Analytics / instrumentation | Deferred intentionally | `METRIC-01` | Product overlay omitted. |
| Testing strategy | Specified | `TEST-01` | Verification plan is strong enough for handoff. |
| Dependencies / integration points | Specified | `INT-01`, `INT-02`, `INT-03` | The key integration seams are well named. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: existing code/docs are sufficient to judge proposal readiness without a new runtime replay.
- ASSUMP-02: implementation will actually carry the newly specified `Artifact` delta through model, pathing, and report surfaces.
- ASSUMP-03: the proposal-local research pack remained fresh enough for this round because the draft changed only by adopting those already-bounded recommendations.
- QUESTION-01: none proposal-blocking in the current reread
- BLOCKER-01: none

## O. Research Triggers / External Questions

- `RQ-01` Host-system integration risk: do authoritative workflow systems preserve previous attempt history/evidence when rerunning failed work from the same logical snapshot, and what can Proposal 013 borrow here without violating current repo truth?
- `RQ-02` Unresolved tradeoff: should validation failure evidence be persisted as a first-class result object distinct from metrics or summary state, and how should operator-facing links/references be derived from that object?
- `RQ-03` Unresolved tradeoff: should output-contract / validation mismatches be treated more like permanent or operator-actionable failures than transient auto-retry cases?
