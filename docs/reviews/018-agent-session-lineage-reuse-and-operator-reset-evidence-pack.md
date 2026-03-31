# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.md` | 2026-03-30 | High | Current draft now explicitly closes the three research-driven gaps: family reuse fail-closes on static-prefix drift, budget policy is metric-driven, and checkpoints preserve continuation-safe rehydration state. | Review could incorrectly keep stale amber findings alive after the draft changed again. | Primary document under review. |
| DOC-02 | prior review: `docs/reviews/018-agent-session-lineage-reuse-and-operator-reset-review.md` | 2026-03-30 | High | The immediate prior pass was `Amber` because family reuse compatibility, budget economics, and checkpoint continuity were still under-specified. | Review could lose the exact delta that mattered this round. | Supports delta analysis. |
| DOC-03 | prior evidence pack: `docs/reviews/018-agent-session-lineage-reuse-and-operator-reset-evidence-pack.md` | 2026-03-30 | High | Earlier evidence already mapped the runtime, lineage, operator, and research seams. | Review could duplicate work or drift from established repo reality. | Supports targeted refresh only. |
| DOC-04 | proposal-local research pack: `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.review/research-pack.md` | 2026-03-30 | High | The fresh `R2` research pack tightened the acceptance bar for family-reuse compatibility, budget economics, and checkpoint rehydration. | Review could miss whether the updated draft actually absorbed those deltas. | Supports the current delta-to-green confirmation. |
| DOC-05 | `.review-baselines/current-system-baseline.md` | 2026-03-30 | High | The repo has a reusable baseline for stable runtime, operator, and proof-lane context. | Review could mis-treat baseline intake as missing. | Primary review accelerator. |
| DOC-06 | `docs/reference/live-provider-execution-slice.md` | 2026-03-30 | High | Current live runtime still states one live session per `AgentExecution`, no session reuse across agents/iterations, and no hidden provider-memory dependency. | Review could miss the exact seam P018 is revising. | Anchors current live-session baseline. |
| DOC-07 | `docs/reference/runtime-contract.md` | 2026-03-30 | High | Current runtime truth still depends on immutable attempt artifacts and frozen run snapshots. | Review could understate the cost of ambiguous branch/session truth. | Anchors persistence and replay expectations. |
| DOC-08 | `docs/reference/execution-truth-and-recovery.md` | 2026-03-30 | High | Current repo already has canonical persisted owners for stage truth, approval truth, and recovery read order. | Review could miss whether P018 still stays downstream of execution truth. | Anchors lineage and recovery authority expectations. |
| DOC-09 | `docs/reference/operator-experience.md` | 2026-03-30 | High | Current operator recovery/actions are shell-owned by `RunsHomeView`, `RecoverySheet`, blocked-run surfaces, reports, and run detail. | Review could miss whether the UI ownership fix stayed intact. | Anchors operator-surface ownership. |
| DOC-10 | `docs/proposals/015-skill-resolution-and-runtime-injection.md` | 2026-03-30 | High | Skill-content hashing and runtime skill injection already have a named proposal owner. | Review could misstate how the P018 binding fingerprint depends on skill truth. | Confirms dependency boundary. |
| DOC-11 | `examples/workflows/full-mvp-live.yaml` | 2026-03-30 | High | The same agent ID already appears under materially different task names and input sets inside one run. | Review could misjudge whether invocation-owner narrowing and family fail-close remain grounded. | Grounds reuse-owner narrowing in current workflows. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level host-system baseline | 2026-03-30 | High | Fresh for repo topology and stable ownership. | Primary review accelerator. |
| BASE-02 | targeted lineage / recovery seam refresh | Reused | stage lineage, approval lineage, retry owners, operator recovery owners | 2026-03-30 | High | No new repo-local refresh was needed beyond the existing seam map plus the fresh research pack. | Supports a defensible delta review. |
| BASE-03 | `<proposal>.review/integration-context.md` | Missing | proposal-local context slice | 2026-03-30 | High | Still missing, but not blocking for this round. | Optional future review accelerator. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- Round classification: delta-to-green round after the prior `Amber` reread and the already-completed deeper `R2` research pass
- Fresh proposal / repo delta:
  - proposal hash changed from `223047f75b45f2b591c6c44b951c069d` to `7913d4ecb8e3697272e1ef53b2469f0c`
  - section `6.1` now expands the binding fingerprint with static scaffold and tool-contract compatibility
  - section `6.2` now makes `sessionFamilyID` insufficient on its own and explicitly fail-closes family reuse on prefix drift
  - section `6.3` now makes caps guardrails and moves decision authority to measured reuse economics
  - section `6.4` now upgrades checkpoints into continuation-safe rehydration artifacts
- In scope:
  - session-lineage ownership
  - reuse invalidation semantics
  - budget-driven compaction / refresh semantics
  - checkpoint-driven fresh rehydration
  - persisted lineage/execution/reset truth
  - operator reset and visibility surfaces
- Out of scope:
  - runtime implementation
  - provider-specific API details
  - performance benchmarking
  - build/run attempts as a default readiness gate
- Deferred intentionally:
  - explicit proof-lane design
  - migration and rollout details
  - implementation-plan decomposition
- Main result:
  - the prior amber blockers are now closed

## D. Impacted Modules / Code-Path Map
| Evidence ID | Module / Surface | Current Role | Verified On | Confidence | Why It Matters |
|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/StageExecution.swift` | current stage-truth owner with `lineageID` and `activeOwnerToken` | 2026-03-30 | High | Confirms the proposal still stays downstream of stage-truth ownership. |
| MAP-02 | `Chainworks Forge/Models/AgentExecution.swift` | current historical execution-truth owner with agent retry supersession fields | 2026-03-30 | High | Confirms session lineage still cannot become a competing execution-truth owner. |
| MAP-03 | `Chainworks Forge/Models/Approval.swift` | current approval-lineage owner | 2026-03-30 | High | Confirms approval lineage remains part of persisted branch truth. |
| MAP-04 | `Chainworks Forge/Engine/RecoveryCoordinator.swift` | current shell recovery-action owner | 2026-03-30 | High | Confirms reset ownership remains grounded. |
| MAP-05 | `Chainworks Forge/Engine/StageRetryCoordinator.swift` | current narrowest-valid-next-action owner | 2026-03-30 | High | Confirms reset remains scoped into the right recovery-policy family. |
| MAP-06 | `Chainworks Forge/Views/RecoverySheet.swift` | current shell-owned recovery action surface | 2026-03-30 | High | Confirms UI ownership remains grounded. |
| MAP-07 | `Chainworks Forge/Views/BlockedRunRecoveryView.swift` | current shell-owned blocked-run recovery and evidence surface | 2026-03-30 | High | Confirms UI ownership remains grounded. |
| MAP-08 | `examples/workflows/full-mvp-live.yaml` | live workflow example with repeated same-agent invocations under distinct task contracts | 2026-03-30 | High | Confirms family reuse still needs strong compatibility rules. |

## E. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | reuse boundary | current draft says reuse is bounded by immutable `invocationOwnerKey` rather than just same-agent reuse | current workflows do reuse the same agent across distinct task contracts, so this narrowing remains necessary and correct | 2026-03-30 | High | Over-broad reuse finding stays closed. |
| REAL-02 | lineage history | current draft keeps lineage persistence split into owner, immutable generations, and append-only events | current repo truth prefers immutable historical owners over mutable summaries | 2026-03-30 | High | Mutable-row history finding stays closed. |
| REAL-03 | operator ownership | current draft keeps reset/inspection on `RecoveryCoordinator`, `RecoverySheet`, `BlockedRunRecoveryView`, and report/evidence surfaces | current repo already centralizes recovery/report actions there | 2026-03-30 | High | UI ownership finding stays closed. |
| REAL-04 | branch authority | current draft says `ownerExecutionLineageID` is a read-only imported authority from execution truth and must fail closed when that truth is missing or contradictory | current repo already has canonical persisted execution/recovery truth owners and expects downstream consumers to read them rather than invent alternatives | 2026-03-30 | High | Branch-authority blocker stays closed. |
| REAL-05 | family reuse compatibility | current draft now says `sessionFamilyID` alone is insufficient and family reuse must fail closed on static-prefix drift | current workflow/examples already reuse the same agent across distinct task contracts, and provider research requires strict compatibility for safe reuse | 2026-03-30 | High | Prior family-reuse blocker is now closed. |
| REAL-06 | budget / compaction economics | current draft now says caps are guardrails and `ContextBudgetGuard` is driven by measured reuse economics | provider research says token-burn control depends on cache-hit or equivalent telemetry, churn, and fresh-vs-reuse comparison | 2026-03-30 | High | Prior budget-economics blocker is now closed. |
| REAL-07 | checkpoint continuity | current draft now adds next steps, learnings, blockers, and owner/binding context to the checkpoint artifact | durable replay research supports a continuation artifact rather than a summary-only artifact | 2026-03-30 | High | Prior checkpoint-fidelity blocker is now closed. |

## F. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `DOC-06` | live session boundary | Entry problem remains concrete and grounded. |
| Same-agent repeated invocation | Specified | `DOC-01`, `DOC-11`, `REAL-01` | workflow examples, invocation owner | Reuse remains bounded by the right owner concept. |
| Retry / resume reuse | Specified | `DOC-01`, `DOC-08`, `REAL-04` | stage lineage, retry truth, imported owner lineage | Branch-authority gap remains closed. |
| Family reuse across invocation owners | Specified | `DOC-01`, `DOC-04`, `DOC-11`, `REAL-05` | `sessionFamilyID`, binding fingerprint, task-family reuse | The static-prefix fail-close is now explicit enough for handoff. |
| Budget-driven refresh | Specified | `DOC-01`, `DOC-04`, `REAL-06` | budget guard, compaction, fresh dispositions | Budget policy is now metric-driven rather than cap-only. |
| Fresh rehydration via checkpoint | Specified | `DOC-01`, `DOC-04`, `REAL-07` | checkpoint artifact, fresh generation handoff | Checkpoint is now continuation-safe enough for proposal readiness. |
| Operator reset | Specified | `DOC-01`, `DOC-09`, `MAP-04`, `MAP-06`, `MAP-07`, `REAL-03` | shell recovery/report owners | Ownership remains explicit and coherent. |
| Reporting / badges | Specified | `DOC-01`, `DOC-08`, `REAL-02`, `REAL-04` | execution truth, lineage history, branch authority | Historical storage and authority align. |
| Clone-run isolation | Specified | `DOC-01` | run boundary | Clone boundary remains explicit and coherent. |

## G. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01`, `DOC-06` | The motivating inefficiency remains real and bounded. |
| Scope boundaries | Specified | `DOC-01`, `DOC-06`, `DOC-09` | Cross-run and cross-agent boundaries remain clear. |
| Reusable baseline coverage | Specified | `DOC-05`, `BASE-01`, `BASE-02` | Baseline intake remains sufficient. |
| Screen / surface definition | Specified | `DOC-01`, `DOC-09`, `REAL-03` | Shell ownership remains explicit. |
| Navigation / entry points | Specified | `DOC-01`, `DOC-09`, `MAP-04`, `MAP-06`, `MAP-07` | Reset/inspection entry remains anchored to existing shell owners. |
| State handling | Specified | state matrix above | The prior family/budget/checkpoint gaps are now closed. |
| Data / API / contract boundary | Specified | `DOC-01`, `DOC-08`, `DOC-10`, `REAL-04`, `REAL-05` | Authority and compatibility relationships are now explicit enough for handoff. |
| Persistence / storage truth | Specified | `DOC-07`, `DOC-08`, `REAL-02`, `REAL-04`, `REAL-07` | History, authority, and continuation now align. |
| Verification ownership | Deferred intentionally | `DOC-01` | Still intentionally deferred and not a proposal-readiness blocker. |
| Testing strategy | Deferred intentionally | `DOC-01` | Still intentionally deferred and not a proposal-readiness blocker. |

## H. Assumptions, Gaps, and Open Questions

- ASSUMP-01: Proposal 015 remains the future owner of skill-content hash truth referenced by the P018 binding fingerprint.
- ASSUMP-02: current operator-shell ownership principles from `operator-experience.md` continue to apply unless a proposal explicitly overrides them.
- GAP-01: no proposal-local `integration-context.md` exists yet; optional future review accelerator only.
- OPEN-01: none proposal-blocking in the current reread.

## I. Research Reuse Note

- The fresh `R2` proposal-local research pack was reused after a freshness check.
- No new external browsing was needed in this round because the updated proposal directly addressed the already-researched gaps.

## O. Research Triggers

- `TRIG-01`: closed. The branch-authority question was answered by the earlier authority section.
- `TRIG-02`: closed. Budget-driven invalidation now references measured reuse economics explicitly enough for proposal readiness.
- `TRIG-03`: closed. The checkpoint artifact now preserves continuation-safe state for fresh rehydration.
- `TRIG-04`: closed. Family reuse now fail-closes when static-prefix compatibility drifts.
- `TRIG-05`: closed. Success metrics now rest on provider-measurable burn signals rather than guessed transcript growth.
- `TRIG-06`: closed. The three research-driven proposal-text gaps from the prior amber reread are now resolved in the updated draft.
