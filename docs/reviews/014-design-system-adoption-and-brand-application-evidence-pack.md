# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/014-design-system-adoption-and-brand-application.md` | 2026-03-30 | High | The updated draft now frames Proposal 014 as an extension of the implemented shared UI system and proof lane, not a replacement for them. | Review could miss whether the earlier blockers were actually closed in text. | Primary document under review. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-03-30 | High | The reusable baseline already treats the operator shell, UI-quality slice, and design authority as stable repo context. | Review could over-reconstruct already-stable boundaries. | Primary review accelerator. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-03-30 | High | Current repo baseline already includes a documented design-system direction and stable UI-quality reference. | Could misjudge whether the proposal is additive or duplicative. | Cross-check for baseline reuse. |
| DOC-04 | `docs/reference/chainworks_forge_design_kit_v1.md` | 2026-03-30 | High | Design Kit v1 remains the visual authority for brand, colors, typography, iconography, and UI rules. | Proposal readiness could be judged without the real authority document. | Primary design authority. |
| DOC-05 | `docs/reference/ui-quality-and-polish.md` | 2026-03-30 | High | The stable UI-quality contract already defines the bounded shared-primitives slice, owner surfaces, and proof obligations. | Could miss whether Proposal 014 is aligned to the current proof and adoption owners. | Key adjacent baseline. |
| DOC-06 | `docs/reference/test-gates.md` | 2026-03-30 | High | The repo already has canonical approved-host UI gates: `proposal-012`, `proposal-006`, and `ui-smoke`. | Could invent a parallel sign-off lane. | Proof-owner baseline. |
| DOC-07 | `docs/evidence/ui-quality-and-polish-proof.md` | 2026-03-30 | High | The current accepted proof story already uses preview rerenders plus `proposal-006`, `proposal-012`, and `ui-smoke` on the same tree. | Could overstate missing proof infrastructure or ignore existing acceptance evidence. | Confirms current proof contract. |
| DOC-08 | prior review: `docs/reviews/014-design-system-adoption-and-brand-application-review.md` | 2026-03-30 | High | The prior round captured the three blockers this delta needed to close: token authority, proof ownership, and stale rollout phases. | Could miss whether the new round is a real delta-to-green or just a repeat. | Supports delta analysis. |
| DOC-09 | prior evidence pack: `docs/reviews/014-design-system-adoption-and-brand-application-evidence-pack.md` | 2026-03-30 | High | The prior evidence already mapped current token owners, proof owners, and stale phase issues. | Could duplicate work or drift from earlier local truths. | Supports reuse-after-freshness-check. |
| DOC-10 | `docs/proposals/014-design-system-adoption-and-brand-application.review/research-pack.md` | 2026-03-30 | High | The existing research pack already validates the bounded authority, brand-asset, iconography, and accessibility-proof stance against primary Apple guidance. | Could miss whether any previously adopted external guidance drifted stale. | Supports no-delta repeat reuse. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level product shape, reference-doc ownership, design authority | 2026-03-30 | High | Fresh for repo topology and proposal dependency orientation. | Avoids broad remap. |
| BASE-02 | targeted refresh over token-authority / proof-owner / rollout-status seams | Partially refreshed | shared owners, preview owners, UI gates, current adopted slice, empty-state naming | 2026-03-30 | High | Needed because repo-level baseline does not enumerate P014-specific design-system ownership in detail. | Supports a defensible readiness call. |
| BASE-03 | `<proposal>.review/integration-context.md` | Missing | proposal-local context slice | 2026-03-30 | High | Absent, but not blocking because affected surfaces were narrow enough to remap directly. | Optional future accelerator. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- Round classification: no-delta repeat proposal-readiness reread
- In scope:
  - design-kit authority adoption
  - token and primitive ownership
  - rollout sequencing against current adoption status
  - proof-lane alignment to current owners
  - accessibility/trust constraints as written in the proposal
- Out of scope:
  - runtime implementation audit
  - remote UI replay
  - product/KPI overlay
  - external research
- Deferred intentionally:
  - proposal-local integration-context artifact
  - runtime proof refresh
- Main result:
  - proposal hash is unchanged from the prior green round, and fresh local/code/research reuse checks did not reopen any contradictions

## D. Impacted Modules / Code-Path Map
| Evidence ID | Module / Surface | Current Role | Verified On | Confidence | Why It Matters |
|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Support/DesignTokens.swift` | current bounded semantic token authority for status, action, spacing, radius, and typography | 2026-03-30 | High | Proposal now correctly treats it as the base owner instead of replacing it abstractly. |
| MAP-02 | `Chainworks Forge/Support/StatusCapsule.swift` | current shared badge primitive with accessibility support and previews | 2026-03-30 | High | Proposal now keeps it canonical and bounded. |
| MAP-03 | `Chainworks Forge/Support/EmptyStateView.swift` | current owner of the `StyledEmptyState` shared empty-state wrapper | 2026-03-30 | High | Appendix `B` now resolves the earlier naming tension. |
| MAP-04 | `Chainworks Forge/ContentView.swift` | current shell owner with direct UI-test surfaces and preview-backed shell render | 2026-03-30 | High | Proof and shell rollout remain anchored to real owners. |
| MAP-05 | `Chainworks Forge/Views/RunsHomeView.swift` | owner surface in the UI-quality slice with min-window/accessibility proof responsibilities | 2026-03-30 | High | Proposal now treats it as current adopted slice plus completion work. |
| MAP-06 | `Chainworks Forge/Views/IdeaListView.swift` | owner surface for summary chips, min-window proof, and preview-backed shell/readability proof | 2026-03-30 | High | Proposal now rebaselines its current adoption status. |
| MAP-07 | `Chainworks Forge/Views/WorkflowMapView.swift` + `ReleaseGateView.swift` + `DeliveryPreflightReportView.swift` | run-centric/adopter surfaces already using `StatusCapsule` / `DesignTokens` and covered by `proposal-012` | 2026-03-30 | High | Proposal now correctly keeps them on the same proof lane. |
| MAP-08 | `Chainworks Forge/Views/ProviderSettingsView.swift` + `PilotReadinessView.swift` + `FirstRunSetupWizard.swift` + `GooseProviderConnectionAssistantView.swift` | setup/remediation surfaces already using current shared tokens and previews | 2026-03-30 | High | Proposal now treats them as rebaseline-and-complete surfaces, not untouched future adopters. |
| MAP-09 | `Chainworks Forge/Views/ArchivedIdeasView.swift` + `RecoverySheet.swift` | secondary/supporting surfaces already previewed or runtime-owned in the current UI baseline | 2026-03-30 | Medium | Proposal now keeps styling-only ownership explicit for recovery surfaces. |
| MAP-10 | `scripts/test-gate.sh` | current canonical UI proof owners: `proposal-012`, `proposal-006`, and `ui-smoke` | 2026-03-30 | High | Proposal now explicitly anchors verification to these owners. |
| MAP-11 | `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift` | existing 1024x768 and adopter-slice accessibility proof for the UI-quality slice | 2026-03-30 | High | Accessibility/min-window proof is now referenced through the correct owner lane. |
| MAP-12 | current asset lane under `Chainworks Forge/Assets.xcassets` | existing app-icon and brand-adjacent source assets already exist | 2026-03-30 | Medium | Proposal now frames a bounded extension of the asset lane rather than implying an empty starting point. |

## E. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | token authority | proposal now says `DesignTokens`, `StatusCapsule`, and `StyledEmptyState` remain canonical owners unless superseded in place | current repo already uses those owners as the implemented bounded slice | 2026-03-30 | High | Earlier second-authority blocker is closed. |
| REAL-02 | proof ownership | proposal now says it extends preview-backed owners plus `proposal-012`, `proposal-006`, and `ui-smoke` | current repo already treats those as the canonical UI proof lane | 2026-03-30 | High | Earlier proof-lane blocker is closed. |
| REAL-03 | rollout sequencing | proposal now records the current adopted slice at `HEAD` and treats several surfaces as rebaseline-and-complete work | current code already applies `DesignTokens` / `StatusCapsule` across shell, run-centric, and setup/remediation surfaces | 2026-03-30 | High | Earlier stale-phase blocker is closed. |
| REAL-04 | recovery boundary | proposal now states that recovery/failed-stage surfaces are styling-only for P014 and retain runtime behavior ownership elsewhere | current repo already owns those behaviors in runtime/recovery references and views | 2026-03-30 | High | Prevents behavior-scope drift. |
| REAL-05 | brand direction | proposal continues to treat Design Kit v1 as authority | current baseline agrees that design-kit direction exists and should govern brand/UI rollout | 2026-03-30 | High | Remains aligned. |

## F. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry / landing shell | Specified | `DOC-01`, `MAP-04`, `MAP-05` | `ContentView`, `RunsHomeView` | Shell ownership and completion path are now clearly grounded. |
| Happy path visual hierarchy | Specified | `DOC-01`, `DOC-04`, `MAP-05`, `MAP-07` | shell/run surfaces | Hierarchy goals remain clear and grounded. |
| Empty states | Specified | `DOC-01`, `MAP-03`, `MAP-09` | `StyledEmptyState`, archive/supporting surfaces | Appendix `B` closes the earlier primitive-naming ambiguity. |
| Accessibility settings | Specified | `DOC-01`, `DOC-05`, `DOC-06`, `MAP-10`, `MAP-11` | `proposal-012` + owner-surface tests | Proof now aligns with bounded current owners. |
| Keyboard-only flows | Specified | `DOC-01`, `DOC-05`, `DOC-06`, `MAP-10`, `MAP-11` | `ui-smoke`, `proposal-012`, `proposal-006` | Current proof owners are explicitly named. |
| Brand/icon application | Specified | `DOC-01`, `DOC-04`, `MAP-12` | assets + app icon lane | Bounded brand-safe application rules are explicit. |
| Secondary surfaces | Specified | `DOC-01`, `MAP-09`, `REAL-04` | archive/recovery/supporting panels | Styling-only recovery boundary removes scope ambiguity. |

## G. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01`, `DOC-04` | Clear motivation and design authority. |
| Scope boundaries | Specified | `DOC-01`, `DOC-05`, `REAL-04` | Visual-system rollout remains bounded and behavior-safe. |
| Reusable baseline coverage | Specified | `DOC-02`, `DOC-03`, `DOC-05` | Baseline intake is explicit and now honored by the draft. |
| Screen / surface definition | Specified | `DOC-01`, `MAP-04`..`MAP-09`, `REAL-03` | Surface list is rebaselined to current adoption status. |
| Navigation / entry points | Specified | `MAP-04`, `MAP-10` | Entry and proof owners remain grounded in real current paths. |
| State handling | Specified | `F`, `REAL-02`, `REAL-04` | Accessibility/trust proof and recovery boundary are explicit. |
| Data / API contract | Deferred intentionally | `DOC-01` | Proposal is UI-system focused. |
| Persistence / caching | Deferred intentionally | `DOC-01` | Out of slice. |
| Feature flags / rollout / rollback | Deferred intentionally | `DOC-01` | Not required for this bounded rollout plan. |
| Analytics / instrumentation | Deferred intentionally | `DOC-01` | Product overlay not requested. |
| Testing strategy | Specified | `DOC-01`, `DOC-06`, `DOC-07`, `MAP-10`, `MAP-11` | Verification is now explicitly attached to the live proof lane. |
| Dependencies / integration points | Specified | `DOC-01`..`DOC-07`, `MAP-01`..`MAP-12` | Design authority, current owners, and proof dependencies are now coherent. |

## H. Assumptions, Gaps, and Open Questions

- ASSUMP-01: proposal readiness can be judged from proposal/doc/code/baseline evidence without a fresh UI replay.
- GAP-01: no proposal-local `integration-context.md` exists yet; non-blocking for this round because affected surfaces were narrow enough to remap directly.
- OPEN-01: none proposal-blocking in the updated draft.

## I. Research Reuse Note

- No new external research was needed in this round.
- Research artifact reused after freshness check: `docs/proposals/014-design-system-adoption-and-brand-application.review/research-pack.md`
- Prior primary Apple platform / accessibility / design guidance remains applicable and did not reopen any local blockers.

## O. Research Triggers / External Questions

| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | `DOC-01`, `MAP-01`, `MAP-02`, `REAL-01` | For an existing macOS app that already has bounded shared owners, what do official Apple design docs suggest about extending a current design system without fragmenting platform consistency or icon semantics? | Local evidence proves repo alignment, but external platform conventions help validate the one-authority extension strategy. | Medium |
| RSH-02 | Host-system integration risk | `DOC-01`, `DOC-04`, `MAP-07`, `MAP-12`, `REAL-05` | What official Apple guidance constrains where app icons, brand assets, and custom iconography belong in operational macOS UI versus identity anchors? | Local design kit defines brand intent, but platform guidance is needed to keep brand application aligned with native expectations. | Medium |
| RSH-03 | Baseline constraint | `DOC-01`, `DOC-05`, `DOC-06`, `MAP-10`, `MAP-11`, `REAL-02` | Which official Apple accessibility and evaluation criteria are most relevant for the bounded proof lane that Proposal 014 extends: color differentiation, contrast/transparency, VoiceOver labels/order, and keyboard continuity? | Local proof owners are known, but external criteria help sharpen what those proof owners should continue to guarantee. | Medium |
