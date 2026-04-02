# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/reference/proposal-loop-feedback-fidelity-and-rereview.md` | 2026-04-02 | High | Current `022` implementation is now documented as a stable reference contract covering `ReviewCorpusBundle`, backlog carry-forward, writer coverage, and shell-owned proposal-loop visibility. | The review could judge an outdated contract snapshot if the reference doc drifts. | Canonical current contract source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | 2026-04-02 | High | Current repo baseline still treats run report, comparison, recovery, and artifact inspection as shell-owned operator surfaces. | Operator-surface ownership could be mapped to the wrong owner. | Needed to judge Sections 3, 4, 10. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md` | 2026-04-02 | High | Current operator flow still routes trust, recovery, report reading, and artifact inspection through existing shell-owned surfaces rather than proposal-specific standalone UIs. | New proposal surfaces could accidentally fork the operator spine. | Needed to judge report / operator-surface language. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-04-02 | High | Current runtime contract remains anchored to immutable run-start truth, persisted artifacts, and explicit execution packets rather than informal summary-only context. | Handoff ownership and artifact truth could be misjudged. | Needed to judge runtime-handoff alignment. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md` | 2026-04-02 | High | Current live proposal-loop slice is still the motivating vertical path for proposal drafting, review, refine, and approval. | Proposal-loop grounding could be overstated. | Needed to judge whether `P022` is scoped to a real live seam. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-02 | High | Current system already prefers explicit artifact truth, fail-closed behavior, and shell-owned evidence/reporting instead of opaque inference. | Proposal recovery/report expectations could be judged against the wrong baseline. | Needed for `UnresolvedIssueGate` and fail-closed review-corpus rules. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/reference/workflow-execution-engine.md` | 2026-04-02 | High | Current workflow execution remains YAML-driven, state-scoped, and artifact-routed. | YAML/runtime migration seams could be judged incorrectly. | Needed to assess `P022` runtime and YAML changes. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | current product type, shell-owned operator spine, immutable run/report model, proposal/review loop baseline | 2026-04-02 | High | Still fresh for report/comparison/artifact ownership. | Primary reusable baseline. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | subsystem map, run/report ownership, canonical reference-doc map | 2026-04-02 | High | Still fresh for shell-owned report/comparison surfaces. | Confirms host-system owners that `P022` must extend. |
| BASE-03 | Proposal-local integration context | Missing | N/A | 2026-04-02 | High | No prior `P022` integration-context artifact exists; targeted repo refresh remained sufficient. | Explains reuse-after-freshness-check workflow. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - proposal-loop refine handoff fidelity
  - score-lift backlog construction and persistence
  - writer coverage truth
  - targeted reviewer rerun policy
  - proposal-growth metrics
  - report / operator-surface consequences for proposal-loop truth
- Out of scope:
  - implementation audit
  - build/run/simulator proof
  - external research
  - generalized strategy experimentation beyond current `P019` seams
- Deferred intentionally:
  - broad provider/model changes
  - transport / settlement substrate changes already owned by `P016`
  - generic context-strategy experimentation already owned by `P019`
- Assumptions:
  - the missing motivating archive does not block proposal-readiness because the key repo-local loop/handoff seams are directly inspectable on `HEAD`
  - `examples/workflows/proposal-loop-live.yaml` and `examples/agents/agents.yaml` remain the canonical live proposal-loop slice
- Open questions:
  - should the motivating archive be indexed next to the proposal for easier future replay-oriented rereads?
- Blockers:
  - none for proposal-readiness

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RunReportView` | Baseline + targeted refresh | 2026-04-02 | High | Current immutable report, export, sign-off, and strategy-summary surfaces already live here. | Proposal could be judged against the wrong owner if this mapping were stale. | Needed for Sections 3, Layer AG, 10. |
| NAV-02 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-02 | High | Current deterministic comparison UI already owns strategy comparison and recommendation display. | Recommendation / comparison ownership could be mapped incorrectly. | Needed for shell-owned visibility alignment. |
| NAV-03 | Approval-context artifact surfacing in `IdeaListView` | Targeted refresh | 2026-04-02 | High | Current operator artifact prioritization already surfaces proposal revision and review artifacts in the shell. | Proposal-loop backlog/coverage visibility could be specified against the wrong surface. | Needed for approval/report artifact alignment. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/examples/workflows/proposal-loop-live.yaml` | Workflow YAML | Live proposal-loop orchestration | 2026-04-02 | High | Current refine state still passes only `proposal_current` plus `proposal_review_summary`; this remains the concrete same-head seam that `P022` now explicitly retires. | The proposal’s seam grounding could be misstated. | Core proposal seam. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml` | Agent catalog | Declares proposal writer inputs and reviewer outputs | 2026-04-02 | High | Current catalog still declares raw quartet inputs for `proposal_writer`, so the repo-local mismatch is between runtime flow and declared writer contract, not between reviewer output definitions and writer needs. | Current defect could be framed incorrectly. | Core proposal seam. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/StewardConfig.swift` | Strategy config | Current handoff-policy defaults | 2026-04-02 | High | `selective_compression_and_escalation` still requests summarized `proposal_review_all`, which is not a live artifact in the proposal loop; `P022` now explicitly marks that path stale and invalid for refine. | Closure of the stale strategy seam could be overstated. | Core proposal seam. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ContextStrategy.swift` | Runtime handoff compiler | Resolves mandatory, summarized, and lazy artifacts | 2026-04-02 | High | `HandoffCompiler` only forwards artifacts that are actually present, so nonexistent aliases like `proposal_review_all` silently collapse out of the handoff packet. | Strategy alias risk could be understated. | Needed for Section 6 review. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime packet assembly | Materializes mandatory/summarized/lazy handoff into the final execution packet | 2026-04-02 | High | Current runtime already distinguishes mandatory artifacts, summaries, and lazy refs, which makes `P022`’s mandatory refine-corpus contract implementable on current seams. | Handoff feasibility could be misjudged. | Needed for review-corpus fidelity rules. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift` | Runtime orchestration | Builds handoff packets and persists strategy metadata | 2026-04-02 | High | Current orchestrator already owns strategy-aware handoff packet building and report metadata flow. | Proposal could open a parallel handoff owner if this seam were misunderstood. | Needed to judge runtime ownership. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ProposalReviewContractAdapter.swift` | Contract adapter | Canonical structured proposal-review contract bridge | 2026-04-02 | High | Current repo already has normalized structured review outputs for the raw quartet and summary, which grounds `ReviewIssueNormalizer` and backlog construction in current structured truth. | Proposal feasibility could be overstated. | Grounds architecture fit. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/OutputContractTemplates.swift` | Output contracts | Current structured review-summary contract definitions | 2026-04-02 | High | Existing contracts already separate raw reviewer reviews and aggregate summary, so the proposal no longer contradicts current contract ownership. | Contract alignment could be judged incorrectly. | Supports readiness. |
| MAP-09 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift` | Operator shell UI | Shell-owned report + strategy tabs | 2026-04-02 | High | Current report UI already owns immutable history, export hub, sign-off, and strategy summary. | Proposal could be accused of forking operator visibility if this owner were stale. | Supports Layer AG alignment. |
| MAP-10 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift` | Operator shell UI | Shell-owned deterministic comparison lane | 2026-04-02 | High | Current comparison UI already owns recommendation, proof owner, and evaluation-set framing for run comparisons. | Comparison alignment could be mapped incorrectly. | Supports Layer AG alignment. |
| MAP-11 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift` | Operator shell UI | Approval-context artifact ordering | 2026-04-02 | High | Current operator approval context already lives in the shell-owned artifact spine; `P022` now explicitly extends this lane instead of inventing a new console. | Approval-context alignment could be overstated. | Supports shell-owned visibility alignment. |
| MAP-12 | `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal019Tests.swift` | Test reality | Current strategy assumptions in repo tests | 2026-04-02 | Medium | Current strategy tests still encode `proposal_review_all` expectations, which confirms the stale alias is still a real same-head seam that the updated proposal now truthfully calls out. | The seam-retirement language could be understated. | Strengthens readiness call. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Proposal-review artifacts | `examples/agents/agents.yaml`, `ProposalReviewContractAdapter.swift`, `OutputContractTemplates.swift` | Produced by reviewer fan-out, consumed by aggregation/refine | 2026-04-02 | High | Current system already persists distinct raw reviewer artifacts plus aggregate summary, giving `P022` a concrete structured source for backlog building. | ReviewIssueNormalizer scope could be overstated. | Core data seam. |
| DATA-02 | Strategy-aware handoff packet | `ContextStrategy.swift`, `WorkflowOrchestrator.swift`, `GooseSessionBridge.swift` | Consumed on every execution | 2026-04-02 | High | Current runtime can distinguish mandatory, summarized, and lazy artifacts, so the proposal’s canonical mandatory refine contract fits current execution machinery. | Runtime fit could be misjudged. | Core runtime seam. |
| DATA-03 | Run/report metadata lane | `RunReportView.swift`, `RunComparisonView.swift`, `Run.swift` | Persisted run/report/comparison truth | 2026-04-02 | High | Current shell already has one report/comparison truth lane for strategy/recommendation metadata, and the proposal now explicitly extends that shell instead of opening a second lane. | Operator ownership could be misunderstood. | Core operator seam. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Alignment Result | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Proposal-loop refine handoff | Current repo | 2026-04-02 | High | Live workflow refine step is summary-only even though the writer catalog declaration includes the raw quartet. | Proposal now explicitly names and retires this seam. | Architecture closure. |
| INT-02 | Strategy profile for selective handoff | Current repo | 2026-04-02 | High | Current selective strategy still tries to summarize `proposal_review_all`, which does not exist in the live proposal loop. | Proposal now explicitly marks this path stale and invalid for refine. | Architecture closure. |
| INT-03 | Operator shell ownership | Baseline + current repo | 2026-04-02 | High | Current operator truth remains shell-owned through report, comparison, recovery, and artifact surfaces. | Proposal now explicitly extends the current shell-owned owners. | UI / UX closure. |
| INT-04 | Existing review artifact prioritization | Current repo | 2026-04-02 | High | Current operator artifact ordering still prioritizes revision summary and aggregate review summary ahead of raw quartet. | Proposal now explicitly uses approval-context artifact surfacing as an extension point for backlog/coverage visibility. | Operator-surface closure. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-01, MAP-02 | proposal loop YAML + agent catalog | Proposal entry and scope are clear. |
| Happy path | Specified | DOC-01, MAP-01..06, INT-01..02 | refine handoff + strategy compiler | The current text now explicitly closes both same-head handoff seams. |
| Loading | Deferred intentionally | DOC-01 | N/A | Proposal is about loop truth, not UI loading states. |
| Empty | Deferred intentionally | DOC-01 | N/A | No separate empty-state UI contract is needed for proposal-readiness. |
| Validation error | Specified | DOC-01, MAP-04..06 | runtime handoff guard | Fail-closed refine behavior is clearly required. |
| Backend error | Specified | DOC-01, DOC-06, INT-03 | execution/report shell | Recovery/report implications now sit on named shell-owned surfaces. |
| Offline / degraded | Deferred intentionally | DOC-01 | N/A | Out of scope for this proposal slice. |
| Retry / recovery | Specified | DOC-01, DOC-06, INT-03 | report/recovery shell | `UnresolvedIssueGate` now extends current shell-owned recovery/report surfaces rather than an unnamed lane. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | N/A | Not part of this proposal. |
| Rollback / cancellation | Deferred intentionally | DOC-01 | N/A | Not part of this proposal. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | Whole slice | Missing | Missing | 2026-04-02 | Medium | Not a proposal-readiness blocker because the slice is proposal-loop-internal and bounded. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `iterations_to_target_score`, `writer_input_payload_bytes`, `issue_drop_rate`, `replan_or_split_trigger_count`, related `P022` metrics | Measure whether full-corpus fidelity and backlog truth improve convergence | Post-review, post-refine, and shell-owned report/comparison surfaces | 2026-04-02 | High | Metrics remain well specified and are now explicitly anchored to the current shell-owned visibility lane. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-defined | review corpus, backlog construction, coverage validation, rerun decisions, growth guard, app-level loop proof, motivating replay proof | Current repo has no `P022`-specific tests yet | Proposal defines unit/integration proof plus app-level and motivating-run replay proof on the canonical proposal-loop path | 2026-04-02 | High | No live proposal-text testing gap remains. |

## L. Current Repo Reality / Alignment
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Result |
|---|---|---|---|---|---|---|
| REAL-01 | Root-cause framing around refine handoff | Current defect is a runtime handoff / review-corpus fidelity problem with two same-head seams that must be retired together | Current repo reality matches that framing: live refine is summary-only and selective strategy still references `proposal_review_all` | 2026-04-02 | High | Aligned |
| REAL-02 | Operator/report visibility | Proposal-loop answers must be explicit in current shell-owned operator surfaces | Current repo already has shell-owned report/comparison/artifact owners for that truth | 2026-04-02 | High | Aligned |
| REAL-03 | Reviewer raw-vs-summary contract | Aggregate summary must not replace raw quartet, and `proposal_review_all` is non-canonical | Current structured-review contract already separates raw quartet and summary, and no canonical `proposal_review_all` artifact exists | 2026-04-02 | High | Aligned |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-05, REAL-01 | Motivation is strong and grounded in a real live proposal-loop defect class. |
| Scope boundaries | Specified | DOC-01 | In-scope and out-of-scope sections are clear. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, INT-01..04 | Proposal now clearly targets current shell/review/runtime seams. |
| Screen / surface definition | Specified | DOC-01, NAV-01..03, INT-03..04 | Proposal now names concrete shell-owned visibility owners. |
| Navigation / entry points | Specified | NAV-01..03 | Existing shell owners are explicitly anchored in text. |
| State handling | Specified | H matrix | Relevant proposal-owned states are adequately covered. |
| Data / API contract | Specified | DATA-01..03, REAL-03 | Review corpus, backlog, coverage, and metrics contracts are well specified. |
| Persistence / caching | Specified | DATA-01..03 | Proposal defines persisted bundle/backlog/coverage truth clearly enough for readiness. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Explicitly out of scope. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Not blocking for proposal-readiness. |
| Analytics / instrumentation | Specified | DOC-01, METRIC-01 | Metric set is explicit and meaningful. |
| Testing strategy | Specified | TEST-01 | Coverage plan is explicit and aligned. |
| Dependencies / integration points | Specified | MAP-01..12, INT-01..04, REAL-01..03 | Current repo seams and proposal seams now match. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The missing motivating archive `D09B432F-D2E7-457B-A61D-6329D78046AD` does not block proposal-readiness because the current YAML, strategy, and shell-owner seams are directly inspectable on `HEAD`.
- ASSUMP-02: The proposal’s quoted score progression and issue counts are accepted as proposal-owned evidence for motivation, not independently replayed during this review pass.
- QUESTION-01: Should the motivating archive be indexed next to the proposal for easier future replay-oriented rereads?
- BLOCKER-01: None

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | None currently required | `None` | None | Local repo evidence is already sufficient to make the readiness call. | Low |
