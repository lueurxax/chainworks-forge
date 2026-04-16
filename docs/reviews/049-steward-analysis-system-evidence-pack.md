# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/049-steward-analysis-system.md` | 2026-04-15 | High | The current working-tree draft closes the older no-signal dossier, deterministic-artifact, and daemon-current-input blockers, but it still names `db/migrations/005_steward.sql` and still mixes `BTreeMap` and `HashMap` for thresholds. | Review would either replay stale blockers or miss the remaining live seams. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should anchor to the reusable repo baseline before judging current-head Steward scope. | Review could drift from current system assumptions. | Intake baseline. |
| DOC-03 | `docs/reference/forge-steward.md` | 2026-04-15 | High | Stable Swift Steward V1 still uses bounded no-signal dossiers, `context` run links, and deterministic artifact writing, and the current proposal now matches those behaviors. | Review could preserve stale parity findings that the current draft already closed. | Stable parity anchor. |
| DOC-04 | prior `docs/reviews/049-steward-analysis-system-review.md` and `docs/reviews/049-steward-analysis-system-evidence-pack.md` | 2026-04-15 | High | Earlier `Red` findings about missing daemon input ownership, missing canonical JSON ownership, and missing no-signal dossier fallback are stale against the current draft. | Review could grade the proposal against an outdated basis. | Stale-basis comparator. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | proposal-review intake assumptions | 2026-04-15 | High | Fresh enough as the shared intake baseline. | Entry baseline. |
| BASE-02 | `docs/reference/forge-steward.md` | Partially refreshed | stable Steward V1 pipeline, dossier behavior, trigger model | 2026-04-15 | High | Refreshed narrowly against the live Swift Steward code and current `P049` text. | Main parity baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - Rust Steward V1 parity for metrics, anomalies, cohorting, dossiers, recommendations, and triggers
  - run-owned cohort / provenance bridge
  - daemon-owned current Steward inputs
  - optional `system_steward` and `steward_auditor` lanes
- Out of scope:
  - Steward UI
  - V2 recommender / V3 experimenter
  - runtime consumption of `context_strategy_profiles`
  - implementation audit
- Assumptions:
  - review mode is `proposal-readiness`
  - the current working-tree draft is the source of truth, not the older review artifacts
  - migration numbering must respect the already-landed Rust schema chain
- Open questions:
  - whether the draft should lock `007_steward.sql` specifically or say "next available migration number at landing time"
  - whether thresholds are intended to stay `BTreeMap` end-to-end
- Blockers:
  - the draft still names an already-occupied migration slot

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | no dedicated Steward UI in the current shell | Baseline | 2026-04-15 | High | `P049` remains an engine / daemon proposal, not a UI change proposal. | Review could invent shell-level blockers that are out of scope. | Keeps review architecture-focused. |
| NAV-02 | daemon startup / current-input owner path | Targeted refresh | 2026-04-15 | High | Current daemon startup is still minimal, and the draft now correctly introduces the missing Steward-owned config/catalog path. | Prevents replaying the stale daemon-owner blocker. | Closure note. |
| NAV-03 | DB migration baseline | Targeted refresh | 2026-04-15 | High | The current DB chain already occupies `005` and `006`. | Proposal can still hand implementation a broken migration filename. | Live blocker proof. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/Run.swift` | Swift model | stable run-owned cohort / provenance fields | 2026-04-15 | High | Swift `Run` already persists `workflowFamily`, `projectKey`, `riskClass`, `stack`, `workflowSnapshotHash`, `catalogSnapshotHash`, and frozen snapshot JSON fields. | The proposal could mis-state the current owner model. | Parity proof. |
| MAP-02 | `Chainworks Forge/Models/RunRepository.swift` | Swift repository | stable run creation owner chain | 2026-04-15 | High | Swift `createRunFromPlan(...)` already derives / persists Steward cohort metadata at run creation time. | The proposal could under-specify the Rust run-owner bridge. | Parity proof. |
| MAP-03 | `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift` | Swift engine | stable dossier fallback, deterministic artifacts, run-link behavior | 2026-04-15 | High | Stable Swift still builds bounded context dossiers when no runs are implicated and writes deterministic JSON with sorted keys. | The proposal could still be judged against stale parity blockers. | Parity proof. |
| MAP-04 | `control-plane/crates/domain/src/run.rs` | Rust domain | current Rust run schema baseline | 2026-04-15 | High | Rust `Run` still lacks the Steward cohort / provenance fields today, so the proposal's widening work remains live. | Review could overstate current Rust parity. | Scope proof. |
| MAP-05 | `control-plane/crates/domain/src/commands.rs` | Rust domain | current run-start contract | 2026-04-15 | High | `StartRunCmd` still only carries per-run workflow/catalog YAML paths today. | Confirms the proposal's new run-owner bridge is still necessary. | Scope proof. |
| MAP-06 | `control-plane/crates/db/src/work_item.rs` | Rust DB / queue | current work-item baseline | 2026-04-15 | High | `WorkItemKind` still has no `StewardAnalysis` variant today. | Confirms the proposal's queue-wiring work is still necessary. | Scope proof. |
| MAP-07 | `control-plane/crates/daemon/src/main.rs` | Rust daemon | current startup baseline | 2026-04-15 | High | Current daemon startup still only reads `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`. | Confirms the draft's new `StewardRuntimeInputs` owner is still solving a live gap. | Scope proof. |
| MAP-08 | `control-plane/crates/db/migrations/005_validation_records.sql` and `control-plane/crates/db/migrations/006_session_lineage.sql` | Rust DB | current migration-chain baseline | 2026-04-15 | High | The existing schema chain already uses `005` and `006`. | Directly proves the draft's `005_steward.sql` collision. | Live blocker proof. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | run-owned cohort / provenance persistence | `Chainworks Forge/Models/Run.swift`, `Chainworks Forge/Models/RunRepository.swift`, `control-plane/crates/domain/src/run.rs` | write at run start, read during Steward analysis | 2026-04-15 | High | Swift already freezes these fields on the run; Rust does not yet. | The proposal could still choose the wrong owner boundary. | Core parity seam. |
| DATA-02 | daemon-owned current Steward inputs | `docs/proposals/049-steward-analysis-system.md`, `control-plane/crates/daemon/src/main.rs` | startup load and shared runtime use | 2026-04-15 | High | The current draft now introduces an explicit owner for current config/catalog inputs that the daemon lacks today. | Review could preserve a stale blocker. | Closed seam. |
| DATA-03 | Steward analysis queue item | `docs/proposals/049-steward-analysis-system.md`, `control-plane/crates/db/src/work_item.rs` | enqueue on post-run / config-change / manual trigger | 2026-04-15 | High | The current draft now converges all trigger paths on one `WorkItemKind::StewardAnalysis` lane. | Review could preserve a stale trigger-path blocker. | Closed seam. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | stable run-owner bridge | Baseline + current repo | 2026-04-15 | High | Swift already freezes Steward cohort / provenance inputs on the run at creation time. | The proposal must match this owner model to claim V1 parity. | Closed seam. |
| INT-02 | daemon-owned current Steward inputs | Proposal + current repo | 2026-04-15 | High | The draft now names a concrete daemon owner for current `StewardConfig` and current `AgentCatalogFile`. | Earlier review blockers on this seam are now stale. | Closed seam. |
| INT-03 | shared queue / executor trigger path | Proposal + current repo | 2026-04-15 | High | The draft now routes post-run, config-change, and manual analysis through one `WorkItemKind::StewardAnalysis` lane. | Earlier review blockers on direct / split execution lanes are now stale. | Closed seam. |
| INT-04 | DB migration sequencing | Current repo | 2026-04-15 | High | The current Rust migration baseline already occupies `005` and `006`. | The proposal still names a colliding migration file. | Live blocker. |
| INT-05 | thresholds container typing | Proposal | 2026-04-15 | High | The draft hardened `StewardConfig.thresholds` to `BTreeMap`, but `detect(...)` still takes `HashMap`. | The proposal remains internally inconsistent on one type boundary. | Low-severity cleanup. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| run-owned cohort / provenance bridge | Specified | DOC-01, MAP-01, MAP-02, MAP-04, DATA-01, INT-01 | Swift `Run`, Swift `RunRepository`, Rust `domain::Run` | Older owner-chain blocker is closed. |
| no-signal context-dossier fallback | Specified | DOC-01, DOC-03, MAP-03 | Swift Steward V1 | Older evidence-completeness blocker is closed. |
| deterministic artifact writing | Specified | DOC-01, DOC-03, MAP-03 | canonical writer + `BTreeMap` contract in proposal | Older determinism blocker is closed. |
| daemon-owned current Steward inputs | Specified | DOC-01, MAP-07, DATA-02, INT-02 | daemon startup | Older startup/manual-trigger blocker is closed. |
| shared trigger queue path | Specified | DOC-01, MAP-06, DATA-03, INT-03 | work-item queue | Older split-path blocker is closed. |
| DB migration plan | Contradicted by repo | DOC-01, MAP-08, INT-04 | migration chain | The proposal still names an occupied slot. |
| thresholds type consistency | Partial | DOC-01, INT-05 | proposal snippets only | Low-level internal cleanup still needed. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none | Steward V1 is proposed as a direct engine / daemon capability | not specified as flag-gated | standard proposal / implementation sequencing | 2026-04-15 | Medium | No flag-specific blocker was found in the proposal. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Steward metrics snapshot + degradation signals | health observability across runs | `steward.run_analysis` and automatic triggers | 2026-04-15 | High | The proposal already defines the core observability outputs; no separate instrumentation blocker was found. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal acceptance + gate | control-plane Steward implementation | the draft includes `proposal-049|p049` with `cargo test --workspace` | update the migration/file-inventory wording so proof and implementation stay aligned | 2026-04-15 | High | The main remaining proof risk is migration-plan drift, not missing test intent. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | no-signal dossiers / `context` links | the current draft now restores bounded no-signal context dossiers | stable Swift Steward V1 already behaves that way | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-02 | daemon-owned current inputs | the current draft now defines `StewardRuntimeInputs` and explicit current config/catalog owners | current daemon still lacks those owners today | 2026-04-15 | High | Earlier blocker is stale and the proposal is now correctly scoped. |
| REAL-03 | deterministic JSON artifacts | the current draft now names deterministic containers and a canonical writer | stable Swift already writes sorted deterministic JSON | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-04 | DB migration numbering | the current draft still names `db/migrations/005_steward.sql` | current Rust DB chain already contains `005_validation_records.sql` and `006_session_lineage.sql` | 2026-04-15 | High | One live proposal blocker remains. |
| REAL-05 | thresholds container type | the current draft mixes `BTreeMap` and `HashMap` for the same thresholds owner boundary | no repo evidence requires that split; it is a proposal-internal inconsistency | 2026-04-15 | High | One low-severity cleanup remains. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-03 | The missing Rust Steward slice is still a live need. |
| Scope boundaries | Specified | DOC-01 | Scope is clear and matches Steward V1 parity intent. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02 | The review has enough stable baseline to judge readiness. |
| Screen / surface definition | Specified | NAV-01, NAV-02 | No hidden UI surface is required here. |
| Navigation / entry points | Specified | NAV-02, DATA-02, DATA-03 | Startup, post-run, and manual trigger entry points are now named. |
| State handling | Specified | H-state rows above | The major previous state gaps are now closed. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-03, INT-05 | Owner chains are specified, but thresholds typing still needs cleanup. |
| Persistence / caching | Partial | MAP-08, INT-04, REAL-04 | Migration-file sequencing still conflicts with the current DB baseline. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Not relevant to Steward V1. |
| Feature flags / rollout / rollback | Deferred intentionally | FLAG-01 | No flag-specific rollout contract is required for proposal readiness here. |
| Analytics / instrumentation | Specified | METRIC-01 | Core observability outputs are defined. |
| Testing strategy | Specified | TEST-01 | The gate exists; wording just needs migration-plan alignment. |
| Dependencies / integration points | Partial | MAP-04, MAP-05, MAP-06, MAP-07, MAP-08 | Integration owners are now named, but migration numbering still needs correction. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The current working-tree proposal text, not the older `049` review artifacts, is the correct review target.
- ASSUMP-02: Migration sequencing in Rust must preserve the already-landed `001` through `006` chain.
- QUESTION-01: Should the proposal lock `007_steward.sql` for the current tree, or describe the migration as "next available slot at landing time"?
- QUESTION-02: Should thresholds stay `BTreeMap` end-to-end, or is the `HashMap` use in `detect(...)` intentional?
- BLOCKER-01: The proposal still names an already-occupied migration file slot.

## O. Research Triggers / External Questions
No external research trigger is required. Local proposal/docs/code/baseline evidence are sufficient for a readiness verdict.
