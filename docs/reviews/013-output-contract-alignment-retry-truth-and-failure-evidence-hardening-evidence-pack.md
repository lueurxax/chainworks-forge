# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` | 2026-03-29 | High | Current working-tree draft keeps the earlier contract / retry / evidence fixes and now tiers the declarative-runtime appendix into Tier `1`, Tier `2`, and Tier `3`. | Review could miss whether the earlier scope blocker is actually closed. | Primary document under review. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-03-29 | High | The repo now has a reusable baseline that already positions runtime contract, operator shell, repo-backed delivery, and sign-off as stable reference areas. | Review could incorrectly keep treating baseline intake as missing. | Primary review accelerator for this round. |
| DOC-03 | `docs/reference/runtime-contract.md` | 2026-03-29 | High | Current runtime truth still requires immutable artifacts per stage attempt and frozen run snapshots. | Could misjudge whether `Section 5.4` still matches current runtime rules. | Anchors retry and storage truth. |
| DOC-04 | `docs/reference/workflow-execution-engine.md` | 2026-03-29 | High | Current persistence seam remains `AgentExecutor -> ArtifactManager -> WorkflowOrchestrator.validateStructuredOutputs(...)`, with stage-attempt-scoped artifact paths. | Could misjudge whether `Section 6.2` still extends the correct implementation seam. | Anchors failure-evidence ordering. |
| DOC-05 | `docs/reference/operator-experience.md` | 2026-03-29 | High | Recovery ownership still lives in `RecoverySheet` and `BlockedRunRecoveryView`. | Could incorrectly reopen the old recovery-ownership finding. | Anchors recovery UX ownership. |
| DOC-06 | `docs/reference/full-mvp-delivery.md` | 2026-03-29 | High | Repo-backed delivery is a stable reference, not a pending proposal dependency. | Could misstate dependency boundaries. | Confirms proposal dependency chain. |
| DOC-07 | `docs/reference/mvp-sign-off.md` | 2026-03-29 | High | MVP sign-off is a stable reference and the correct replacement for removed Proposal 008 dependency paths. | Could keep stale dependency assumptions alive. | Confirms dependency normalization remains correct. |
| DOC-08 | prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md` | 2026-03-29 | High | Prior local review had Proposal 013 `Amber` because Layer `Q` widened too far without a must-land subset. | Could lose track of what actually changed this round. | Supports delta analysis. |
| DOC-09 | prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md` | 2026-03-29 | High | Earlier evidence already mapped the relevant runtime seams and motivating failure class. | Could duplicate work or drift from established repo truth. | Supports targeted refresh only. |
| DOC-10 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md` | 2026-03-29 | High | Research-backed deltas around same-snapshot retry lineage, canonical validation-failure reference truth, and non-auto-retryable mismatch defaults remain adopted in the draft. | Could miss whether the current proposal regressed from prior adopted guidance. | Supports closure confirmation on the old slice. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level host-system baseline | 2026-03-29 | High | Fresh for repo-level topology and dependency-chain orientation. | Replaces the earlier baseline-missing process gap. |
| BASE-02 | targeted declarative / retry / evidence seam refresh | Partially refreshed | Appendix `B` tiering, contract authority, retry storage truth, failure ordering, recovery ownership | 2026-03-29 | High | Needed because the repo-level baseline does not enumerate P013-specific seams in detail. | Supports a defensible proposal-readiness call. |
| BASE-03 | `<proposal>.review/integration-context.md` | Missing | proposal-local context slice | 2026-03-29 | High | Still absent, but not blocking because the targeted refresh stayed narrow. | Optional future review accelerator. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- Round classification: material delta round, but the key scope blocker is now closed.
- Fresh proposal delta:
  - scope now says `declarative YAML coverage audit and contract-tier hardening`
  - Layer `Q` is split into mandatory Tier `1` versus deferred Tier `3`
  - `4.2.2` now limits mandatory implementation work to `contracts.*` and `structured_output`
  - acceptance and verification now bind to Tier `1`, not the whole appendix
  - Appendix `B` now explicitly distinguishes mandatory, metadata-only, and later rows
- In scope:
  - proposal readiness for output-contract alignment, retry truth, failure evidence, blocked-run recovery explanation, and proposal-output compaction
  - validation of the new tiered declarative-runtime appendix against current parser / compiler / transport code
- Out of scope:
  - build/run attempts
  - product KPI overlay
  - provider-family expansion
  - repo-backed delivery implementation details outside the named failure class
- Deferred intentionally:
  - optional proposal-local integration-context artifact
  - fresh external research
  - implementation audit
  - Tier `3` declarative-runtime gaps such as `skill_ref`, `required_tools`, broader transport policy enforcement, and wider workflow coverage
- Main result:
  - the earlier scope-boundary blocker is closed because the new declarative-runtime slice is now explicitly tiered and bounded

## D. Impacted Modules / Code-Path Map
| Evidence ID | Module / Surface | Current Role | Verified On | Confidence | Why It Matters |
|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/DSL/AgentCatalog.swift` | decodes `skill_ref`, `skill_role`, `required_tools`, backend settings, permission profiles, worktree policy, contracts, and descriptive metadata | 2026-03-29 | High | Confirms Appendix `B` starts from real decoded schema surfaces. |
| MAP-02 | `Chainworks Forge/DSL/WorkflowDefinition.swift` + `YAMLValidator.swift` | decodes and validates workflow-level execution, failure-policy, idea-input, and scoring fields; validates referenced skills and permission profiles exist | 2026-03-29 | High | Confirms which YAML surfaces are validated today versus actually executed. |
| MAP-03 | `Chainworks Forge/Engine/RunPlanCompiler.swift` + `RunPlan.swift` | copies `skillRef`, `skillRole`, `permissionProfile`, `maxTurns`, `temperature`, and `worktreeWriteEnabled` into `ResolvedAgent` / `RunPlan` | 2026-03-29 | High | Confirms declarative fields survive compile-time even if only some are Tier `1`. |
| MAP-04 | `Chainworks Forge/Engine/GooseSessionBridge.swift` + `GooseTransport.swift` | forwards `permissionProfileID` and coarse workspace-mode booleans into live session policy, but not `skillRef`, `skillRole`, `required_tools`, `maxTurns`, or `temperature` | 2026-03-29 | High | Confirms why those rows correctly stay Tier `3` rather than silently remaining inside 013 scope. |
| MAP-05 | `Chainworks Forge/Engine/AgentExecutor.swift` | owns current `OutputContractResolver`, including hardcoded output-name fallback branches | 2026-03-29 | High | Confirms output-contract hardening still targets a real Tier `1` drift seam. |
| MAP-06 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | validates structured outputs, hashes `skillRef` into metadata, and computes agent config hashes using runtime settings plus `skillRef` | 2026-03-29 | High | Confirms current repo preserves declarative fields in provenance even when they are not fully enforced live. |
| MAP-07 | `Chainworks Forge/Engine/ArtifactManager.swift` + `ArtifactStorage.swift` | persist current stage-attempt-scoped artifact truth | 2026-03-29 | High | Confirms same-stage agent-retry storage still needs a disjoint namespace. |
| MAP-08 | `Chainworks Forge/Models/Artifact.swift` + `StageExecution.swift` + `AgentExecution.swift` | current persisted retry and artifact truth remain stage-attempt-scoped with no agent-attempt lineage layer | 2026-03-29 | High | Confirms Proposal 013 still names a real model delta. |
| MAP-09 | `Chainworks Forge/Engine/ResumeManager.swift` | uses `requires_human_approval` and some permission-profile names as heuristics | 2026-03-29 | High | Confirms those fields are only partially runtime-authoritative today and belong outside Tier `1`. |
| MAP-10 | `Chainworks Forge/Views/RecoverySheet.swift` + `BlockedRunRecoveryView.swift` | current shell-owned recovery surfaces | 2026-03-29 | High | Confirms recovery UX ownership remains stable. |

## E. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | output-contract authority | typed resolver/schema remain derived from catalog-backed truth | current runtime still resolves contract IDs through `OutputContractResolver` plus `catalog.contracts` | 2026-03-29 | High | Old second-authority finding stays closed. |
| REAL-02 | transitional contract drift | proposal says hardcoded `outputName -> contractID` branches are transitional drift | current `OutputContractResolver` still contains explicit proposal-review and summary mappings | 2026-03-29 | High | Tier `1` hardening remains truthful and implementation-oriented. |
| REAL-03 | `skill_ref` / `skill_role` coverage | proposal now classifies these rows as Tier `3` later work, not 013-mandatory | current code still decodes, validates, carries, and hashes them without injecting them into live Goose execution | 2026-03-29 | High | The new tiering closes the earlier scope problem. |
| REAL-04 | permission and tool policy coverage | proposal now classifies permission allowlists and `required_tools` as Tier `3` later work | current live session policy only passes `permissionProfileID`; `requiredTools` appears only in DSL decoding | 2026-03-29 | High | The new tiering closes the earlier scope problem here too. |
| REAL-05 | backend runtime settings coverage | proposal keeps `provider` / `model` as already used, moves `max_turns`, `temperature`, and `effort` out of 013 scope, and keeps `structured_output` in Tier `1` | current compiler preserves these values, while live transport still ignores all but provider/model-level effect | 2026-03-29 | High | Tiering now matches actual implementation seams. |
| REAL-06 | workflow-level YAML coverage | proposal now treats workflow-level declarative gaps as Tier `2` or Tier `3`, with no workflow Tier `1` rows | current code decodes these surfaces but runtime authority remains uneven | 2026-03-29 | High | The motivating failure class remains correctly contract-driven. |
| REAL-07 | same-stage retry storage | proposal adds explicit agent-retry lineage and disjoint `agent-retry-{agentAttemptNumber}` namespace | current repo still stores artifacts only by stage attempt under `{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}` | 2026-03-29 | High | Proposal still names a real storage delta. |
| REAL-08 | failure evidence ordering | proposal anchors persistence before validation and settlement | current repo still persists through `ArtifactManager` and validates in `WorkflowOrchestrator` after that boundary | 2026-03-29 | High | Old persistence-boundary finding stays closed. |
| REAL-09 | recovery ownership | proposal extends `RecoverySheet` and `BlockedRunRecoveryView` | current repo still uses exactly those shell-owned surfaces | 2026-03-29 | High | Old recovery-ownership finding stays closed. |
| REAL-10 | proposal boundary | proposal now tiers Appendix `B` and limits 013-mandatory implementation to Tier `1` only | current appendix no longer mixes all execution-relevant rows into one flat implementation surface | 2026-03-29 | High | Earlier `Amber` scope blocker is closed. |
| REAL-11 | top-level wording | proposal question `2.6` still sounds broader than the new tiered boundary | acceptance `9-10` and `4.2.2` are already scoped correctly | 2026-03-29 | Medium | Residual wording cleanup only; not proposal-blocking. |

## F. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `MAP-10`, `REAL-09` | blocked-run recovery and report surfaces | Entry to the reviewed slice remains clear and aligned with current shell ownership. |
| Happy path | Specified | `DOC-01`, `REAL-01`, `REAL-07` | successful retry without clone | Retry semantics remain explicit and locally grounded. |
| Loading | Deferred intentionally | `DOC-01` | none material | Proposal 013 is still about persisted truth, not loading behavior. |
| Empty | Partial | `DOC-01`, `MAP-10` | failed-stage evidence panel / recovery surface | Empty-state specificity remains secondary and non-blocking. |
| Validation error | Specified | `DOC-01`, `REAL-01`, `REAL-08` | structured-output validation path | Validation-failure handling remains central and explicit. |
| Backend error | Specified | `DOC-01`, `MAP-07`, `REAL-08` | receipt / transcript / raw-output preservation | Failure-evidence preservation remains explicit. |
| Offline / degraded | Deferred intentionally | `DOC-01` | transport degradation not central here | Proposal 013 is not a general transport-hardening slice. |
| Retry / recovery | Specified | `DOC-01`, `REAL-07`, `REAL-09` | retry lineage and recovery/report surfaces | Action semantics, lineage, and artifact truth remain coherent. |
| Auth / permission expiry | Deferred intentionally | `DOC-01`, `REAL-04` | permission profiles are only partially in scope through declarative audit | Not central to the motivating failure class. |
| Rollback / cancellation | Deferred intentionally | `DOC-01` | clone-run / run settlement only | Still outside this bounded slice. |

## G. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01` | The motivating failure class remains concrete and bounded. |
| Scope boundaries | Specified | `DOC-01`, `REAL-10` | The earlier declarative-runtime scope issue is now closed by Tier `1` / `2` / `3` partitioning. |
| Reusable baseline coverage | Specified | `DOC-02`, `BASE-01`, `BASE-02` | Earlier baseline gap is closed. |
| Screen / surface definition | Specified | `DOC-05`, `MAP-10`, `REAL-09` | Recovery ownership remains clear. |
| Navigation / entry points | Specified | `MAP-10` | No fresh navigation ambiguity surfaced. |
| State handling | Specified | state matrix above | Retry, validation-failure, and clone semantics remain explicit. |
| Data / API / contract boundary | Specified | `MAP-05`, `REAL-01`, `REAL-02`, `REAL-08` | Contract authority and failure ordering remain implementation-ready. |
| Persistence / storage truth | Specified | `MAP-07`, `MAP-08`, `REAL-07`, `REAL-08` | Same-stage agent-retry storage truth remains explicit. |
| YAML / declarative coverage honesty | Specified | `MAP-01`, `MAP-02`, `MAP-03`, `MAP-04`, `REAL-03`, `REAL-04`, `REAL-05`, `REAL-06`, `REAL-10` | Appendix `B` is now truthful and bounded. |
| Mandatory subset for 013 | Specified | `DOC-01`, `REAL-10` | Tier `1` is now explicit and implementation-ready. |
| Feature flags / rollout | Missing | `DOC-01` | Still omitted, but not proposal-blocking in this bounded slice. |
| Analytics / instrumentation | Deferred intentionally | `DOC-01`, `DOC-10` | Product overlay remains out of scope. |
| Testing strategy | Specified | `DOC-01`, `REAL-10` | Verification now binds to Tier `1` instead of the whole appendix. |

## H. Assumptions, Gaps, and Open Questions

- ASSUMP-01: the current repo-level baseline is sufficient for proposal readiness when combined with the narrow P013 seam refresh captured here.
- ASSUMP-02: Tier `2` and Tier `3` appendix rows are intended to keep runtime-truth honest, not to widen implementation scope back out.
- GAP-01: no proposal-local `integration-context.md` exists yet; optional future review hygiene only.
- OPEN-01: none proposal-blocking in the current reread.

## I. Research Reuse Note

- No fresh web research was needed for the proposal-readiness call itself.
- A separate research refresh was completed later on `2026-03-29` in `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`.
- The prior proposal-local research pack remained applicable as a base because the current delta was repo-local and code-checkable, but its source ledger was refreshed and extended for the Tier `1` `structured_output` question.
- Reused research conclusions that still hold:
  - same-run retry should preserve the same frozen logical snapshot while appending inspectable attempt history
  - validation failure should remain a first-class persisted evidence object
  - contract mismatch and post-generation validation failure should remain non-auto-retryable by default unless explicit recovery or policy says otherwise
