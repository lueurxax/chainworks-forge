# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` | 2026-03-30 | High | Current draft keeps the earlier contract / retry / failure-evidence fixes and now requires an app-level proof in Section `9.2`, but does not name a canonical proof owner. | Review could miss that the remaining problem is now proof-lane ownership, not core design. | Primary document under review. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-03-30 | High | The repo has a reusable baseline that already positions runtime contract, operator shell, repo-backed delivery, and sign-off as stable reference areas. | Review could incorrectly treat baseline intake as missing. | Primary review accelerator for this round. |
| DOC-03 | `docs/reference/runtime-contract.md` | 2026-03-30 | High | Current runtime truth still requires immutable artifacts per attempt and frozen run snapshots. | Could misjudge whether retry and artifact rules still match the proposal. | Anchors retry and storage truth. |
| DOC-04 | `docs/reference/workflow-execution-engine.md` | 2026-03-30 | High | Current persistence seam remains executor -> artifact manager -> output validation, and direct surfaces / tests live outside that seam. | Could misjudge whether failure-evidence ordering or proof ownership still maps to real repo seams. | Anchors persistence and proof-lane mapping. |
| DOC-05 | `docs/reference/operator-experience.md` | 2026-03-30 | High | Recovery ownership still lives in `RecoverySheet` and `BlockedRunRecoveryView`. | Could incorrectly reopen the recovery-ownership finding. | Anchors recovery UX ownership. |
| DOC-06 | `docs/reference/full-mvp-delivery.md` | 2026-03-30 | High | Repo-backed delivery remains a stable reference, not a pending proposal dependency. | Could misstate dependency boundaries. | Confirms proposal dependency chain. |
| DOC-07 | `docs/reference/mvp-sign-off.md` | 2026-03-30 | High | MVP sign-off remains a stable reference. | Could keep stale dependency assumptions alive. | Confirms dependency normalization remains correct. |
| DOC-08 | prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md` | 2026-03-30 | High | Prior local review had Proposal 013 green. | Could lose track of what changed in the proposal and current repo since that pass. | Supports delta analysis. |
| DOC-09 | prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md` | 2026-03-30 | High | Earlier evidence already mapped the relevant runtime seams and motivating failure class. | Could duplicate work or drift from established repo truth. | Supports targeted refresh only. |
| DOC-10 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md` | 2026-03-30 | High | Research-backed deltas around same-snapshot retry lineage, canonical validation-failure reference truth, and non-auto-retryable mismatch defaults remain adopted in the draft. | Could miss whether the current proposal regressed from prior adopted guidance. | Supports closure confirmation on the old slice. |
| DOC-11 | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening_IMPLEMENTATION_AUDIT_R6.md` | 2026-03-30 | High | The latest implementation audit shows the remaining blocker is Section `9.2`: a scaffold-only `UITestProposal013EvidenceSurface` exists outside the repo's canonical proof lane. | Could miss the concrete repo-local evidence that the current proof wording is too loose. | Key local trigger for this review delta. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level host-system baseline | 2026-03-30 | High | Fresh for repo-level topology and dependency-chain orientation. | Replaces the earlier baseline-missing process gap. |
| BASE-02 | targeted proof-lane seam refresh | Partially refreshed | direct-surface ownership, UI-test ownership, `test-gate` ownership, latest implementation-audit delta | 2026-03-30 | High | Needed because the repo-level baseline does not enumerate P013-specific proof-lane ownership. | Supports a defensible proposal-readiness call. |
| BASE-03 | `<proposal>.review/integration-context.md` | Missing | proposal-local context slice | 2026-03-30 | High | Still absent, but not blocking because the targeted refresh stayed narrow. | Optional future review accelerator. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- Round classification: material delta round
- Fresh proposal / repo delta:
  - proposal changed since the last green review
  - latest implementation audit `R6` isolated the remaining gap to app-level proof ownership
  - current repo continues to use a stable direct-surface + UI-test + `test-gate` proof lane for proposal-scoped UI evidence
- In scope:
  - proposal readiness for output-contract alignment, retry truth, failure evidence, blocked-run recovery explanation, proposal-output compaction, and app-level proof ownership
  - validation of Section `9.2` against current proof-owner reality
- Out of scope:
  - build/run attempts as a default review gate
  - product KPI overlay
  - implementation completeness beyond what is needed to expose text-level contradictions
- Deferred intentionally:
  - fresh external research
  - implementation audit itself
  - proposal-local integration-context artifact
- Main result:
  - the core design remains coherent, but Section `9.2` is under-specified against current repo proof ownership

## D. Impacted Modules / Code-Path Map
| Evidence ID | Module / Surface | Current Role | Verified On | Confidence | Why It Matters |
|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/OutputContractResolverV2.swift` | canonical runtime reader for contract truth | 2026-03-30 | High | Confirms old contract-authority finding stays closed. |
| MAP-02 | `Chainworks Forge/Engine/OutputContractDeclarativeBridge.swift` | migration verifier comparing legacy and catalog-driven contract binding | 2026-03-30 | High | Confirms current draft still names a real Tier `1` hardening seam without reopening second-authority risk. |
| MAP-03 | `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift` | proposal-review and aggregate summary contract adapter | 2026-03-30 | High | Confirms strict structured review-output intent remains grounded in code. |
| MAP-04 | `Chainworks Forge/Views/UITestDirectSurfaces.swift` | defines proposal-specific direct proof surfaces, including `UITestProposal013EvidenceSurface` | 2026-03-30 | High | Shows the current proof surface that triggered the new finding. |
| MAP-05 | `Chainworks Forge/ContentView.swift` | declares the canonical `UISurface` enum and direct-surface routing used by app-driven UI proof | 2026-03-30 | High | Confirms current repo already has a canonical direct-surface owner boundary. |
| MAP-06 | `Chainworks Forge/Chainworks_ForgeApp.swift` | bootstraps forced UI surfaces for UI tests and direct proof paths | 2026-03-30 | High | Confirms the second half of the canonical app-level proof lane. |
| MAP-07 | `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift` | current UI proof owners for proposal-scoped and direct-surface validation | 2026-03-30 | High | Confirms the accepted UI-proof owner layer in the repo. |
| MAP-08 | `scripts/test-gate.sh` | canonical operator/agent entry point for approved UI proof lanes | 2026-03-30 | High | Confirms current repo has explicit proposal-scoped gate ownership for comparable UI proposals. |

## E. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | contract authority | typed resolver/schema remain derived from catalog-backed truth | current runtime resolves contracts through `OutputContractResolverV2` and catalog-derived schema | 2026-03-30 | High | Old second-authority finding stays closed. |
| REAL-02 | transitional contract drift | proposal says hardcoded output-name binding is transitional drift | current repo isolates legacy comparison logic inside `OutputContractDeclarativeBridge`; runtime readers are V2-based | 2026-03-30 | High | Tier `1` hardening remains truthful and implementation-oriented. |
| REAL-03 | retry and failure-evidence ownership | proposal extends current recovery/evidence seams rather than replacing them | current repo still uses `RecoverySheet`, `BlockedRunRecoveryView`, and `FailedStageEvidencePanel` as stable owners | 2026-03-30 | High | Old recovery/evidence ownership findings stay closed. |
| REAL-04 | canonical direct-surface owner | proposal asks for app-level proof but does not name a proof owner | current repo already has one canonical direct-surface owner boundary in `ContentView.UISurface` and `Chainworks_ForgeApp` forced-surface boot | 2026-03-30 | High | Section `9.2` is now under-specified against current repo reality. |
| REAL-05 | UI test owner layer | proposal asks for app-level proof but does not say whether the proof must be owned by current UI tests | current repo already routes comparable proposal/UI proof through `Chainworks ForgeUITests` | 2026-03-30 | High | Verification authority is ambiguous in the current draft. |
| REAL-06 | gate ownership | proposal asks for app-level proof but does not say whether the proof must live on `test-gate` | current repo already names proposal-scoped gate ownership for adjacent UI proposals in `scripts/test-gate.sh` | 2026-03-30 | High | Verification authority is ambiguous in the current draft. |
| REAL-07 | latest implementation delta | proposal wording is broad enough to ensure canonical incident-closure proof | latest implementation audit `R6` shows a scaffold-only `UITestProposal013EvidenceSurface` outside the canonical lane, and that surface still seeds a blocked run instead of proving the full app-launched story | 2026-03-30 | High | The current draft already allowed acceptance drift in practice. |

## F. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `MAP-05`, `MAP-06` | direct-surface boot and app entry | Entry to the reviewed slice remains clear. |
| Happy path | Specified | `DOC-01`, `MAP-01`, `MAP-03` | contract-valid review outputs and aggregate summary | Core happy path remains bounded and coherent. |
| Validation error | Specified | `DOC-01`, `REAL-03` | failed-stage evidence and narrow recovery | Validation-failure handling remains central and explicit. |
| Retry / recovery | Specified | `DOC-01`, `REAL-03` | recovery UI, failed-stage evidence, clone-vs-retry semantics | Recovery semantics remain explicit and grounded. |
| App-level proof | Partial | `DOC-01`, `DOC-11`, `REAL-04`, `REAL-05`, `REAL-06`, `REAL-07` | direct surfaces, UI tests, `test-gate` | The proof content is specified, but the owner boundary is not. |
| Declarative contract control visibility | Specified | `DOC-01`, `MAP-01`, `MAP-02` | contract and structured-output tiering | Current draft remains truthful on declarative coverage. |

## G. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01`, `DOC-11` | The motivating failure class remains concrete and bounded. |
| Scope boundaries | Specified | `DOC-01`, `REAL-01`, `REAL-02`, `REAL-03` | The design boundary remains coherent. |
| Reusable baseline coverage | Specified | `DOC-02`, `BASE-01`, `BASE-02` | Baseline intake remains sufficient. |
| Screen / surface definition | Specified | `DOC-05`, `REAL-03` | Recovery/evidence surface ownership remains clear. |
| Navigation / entry points | Partial | `MAP-05`, `MAP-06`, `REAL-04` | Core entry path exists in repo, but proposal verification does not explicitly claim it. |
| State handling | Specified | state matrix above | Retry, validation-failure, and clone semantics remain explicit. |
| Data / API / contract boundary | Specified | `MAP-01`, `MAP-02`, `MAP-03`, `REAL-01`, `REAL-02` | Contract authority and Tier `1` hardening remain implementation-ready. |
| Persistence / storage truth | Specified | `DOC-03`, `DOC-04`, `REAL-03` | Failure evidence and retry lineage remain grounded. |
| Verification ownership | Partial | `DOC-01`, `DOC-11`, `REAL-04`, `REAL-05`, `REAL-06`, `REAL-07` | Section `9.2` does not yet name the repo's canonical app-proof lane. |
| Testing strategy | Partial | `DOC-01`, `DOC-11`, `MAP-07`, `MAP-08` | Content is strong, but authority is not explicit. |

## H. Assumptions, Gaps, and Open Questions

- ASSUMP-01: the current repo-level baseline is sufficient for proposal readiness when combined with the narrow proof-lane seam refresh captured here.
- ASSUMP-02: the existing direct-surface + UI-test + `test-gate` owner chain is now the repo's canonical macOS UI proof lane for proposal-scoped acceptance evidence.
- GAP-01: no proposal-local `integration-context.md` exists yet; optional future review hygiene only.
- OPEN-01: none beyond the explicit Section `9.2` proof-owner fix.

## I. Research Reuse Note

- No fresh web research was needed for the proposal-readiness call itself.
- The existing proposal-local research pack remained applicable because the live issue is repo-local proof ownership, not a modern platform question.
- Reused research conclusions that still hold:
  - same-run retry should preserve the same frozen logical snapshot while appending inspectable attempt history
  - validation failure should remain a first-class persisted evidence object
  - contract mismatch and post-generation validation failure should remain non-auto-retryable by default unless explicit recovery or policy says otherwise
