# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md` | 2026-04-01 | High | Current `P015` explicitly aligns external skills to Codex `SKILL.md` bundles, aligns specialization to the real `proposal_review_triad` mode contract, anchors frozen truth to immutable `Run` + `RunStartSnapshot`, uses dual raw/injected hash semantics, and extends shell-owned visibility instead of inventing `AgentInspectorView`. | The review could judge an outdated contract. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/yaml-dsl-parser.md` | 2026-04-01 | High | Current parser surface still ends at `SkillRef` parsing plus referential validation; there is no live runtime skill-resolution layer yet. | Proposal-to-repo delta could be misstated. | Baseline for current YAML/catalog ownership. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-04-01 | High | Immutable run-start truth in current repo continues to be expressed through immutable `Run` fields plus `RunStartSnapshot`. | Frozen-owner alignment could be judged incorrectly. | Needed to verify snapshot alignment. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-01 | High | Skills are still documented as parsed and displayed but not live runtime-authoritative in current implementation. | The motivation for `P015` could be misread. | Confirms the proposal still solves a real gap. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md` | 2026-04-01 | High | Current operator shell ownership still lives in reports, comparison, recovery, and artifact inspection. | UI ownership could be mapped to the wrong surface. | Needed to verify Section 8 alignment. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md` | 2026-04-01 | High | `PilotReadinessView` and related preflight/report surfaces remain the current readiness owners. | Preflight visibility recommendations could drift from current shell reality. | Needed for Section 7 and 8.3 alignment. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | current product type, operator shell, frozen-run baseline, canonical reference-doc map | 2026-04-01 | High | Still fresh for shell and run-start ownership. | Review entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | subsystem map, operator boundary, provider/platform baseline | 2026-04-01 | High | Still fresh for shell/report ownership. | Confirms existing owners that `P015` must extend. |
| BASE-03 | Proposal-local review / integration context | Missing | N/A | 2026-04-01 | High | No extra proposal-local integration context was needed after targeted refresh. | Explains why the reread remained repo-local. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - external / inline / builtin skill resolution semantics
  - runtime injection path
  - `skill_role` / mode behavior
  - raw and injected provenance ownership
  - preflight/readiness ownership
  - shell-owned operator visibility
- Out of scope:
  - implementation audit
  - runtime proof or build/run gating
  - external research
  - full Codex skill-runtime emulation beyond what `P015` explicitly adopts
- Deferred intentionally:
  - lazy or partial injection
  - promotion of companion bundle files into executable runtime truth
  - rollout / flag strategy
- Assumptions:
  - current `.codex/skills/*` bundle shape is the intended external skill MVP source of truth
  - current examples under `/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml` are valid grounding for real role/mode usage
- Open questions:
  - if future proposals promote companion bundle files into executable truth, should flattening happen at compile time or through typed runtime references?
- Blockers:
  - none for proposal-readiness

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `AgentCatalogView` | Targeted refresh | 2026-04-01 | High | Current surface still shows only `skillRef` and optional `skillRole`, so Proposal 015’s catalog-visibility delta is real and bounded. | Visibility delta could be overstated. | Directly touched by Section 8.1. |
| NAV-02 | `PilotReadinessView` + preflight report surfaces | Baseline + targeted refresh | 2026-04-01 | High | Current readiness UI remains the canonical owner for preflight output. | Readiness ownership could drift. | Directly touched by Section 7 and 8.3. |
| NAV-03 | `RunReportView`, `RunComparisonView`, `ArtifactInspectorView` | Baseline + targeted refresh | 2026-04-01 | High | Current execution inspection spine is still shell-owned through report / comparison / artifact routes. | Execution-time visibility could be attached to the wrong owner. | Directly relevant to Section 8.2. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift` | DSL | Parses `skills`, `skill_ref`, `skill_role` | 2026-04-01 | High | `SkillRef` remains parsed data only today. | Proposal seam could be misstated. | Entry point for skill declarations. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/YAMLValidator.swift` | DSL validation | Validates referential integrity | 2026-04-01 | High | Current validation still does not own path/name/content resolution. | Preflight delta could be misjudged. | Needed for Section 7. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlan.swift` | Compiler/runtime boundary | Holds `ResolvedAgent` | 2026-04-01 | High | `ResolvedAgent` currently still lacks resolved skill payloads. | Compiler-layer delta could be understated. | Needed for Layer B/C review. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlanCompiler.swift` | Compiler | Resolves backend profiles into `ResolvedAgent` | 2026-04-01 | High | No live skill loading happens during compilation yet. | Proposal motivation could be misstated. | Core proposal seam. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift` | Runtime orchestration | Persists skill provenance | 2026-04-01 | High | Current provenance still hashes `agent.skillRef` rather than resolved or injected content. | Provenance delta could be misstated. | Core proposal seam. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime packet assembly | Builds canonical execution packet | 2026-04-01 | High | No current skill preamble/injection path exists, and the proposal now explicitly keeps packet ownership here. | Injection-owner alignment could be judged incorrectly. | Core proposal seam. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/AgentCatalogView.swift` | Operator UI | Current catalog inspection surface | 2026-04-01 | High | Current UI still shows ref + role only. | UI delta could be overstated. | Section 8.1 owner. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/PilotReadinessView.swift` | Operator UI | Current readiness owner | 2026-04-01 | High | Current readiness/report ownership already exists and remains shell-owned. | Readiness routing could drift. | Section 7 / 8.3 owner. |
| MAP-09 | `/Users/user/.codex/skills/proposal-review-triad/SKILL.md` and `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md` | External skill packages | Current configured external skill sources | 2026-04-01 | High | Current external skills are Codex bundles rooted at `SKILL.md`, with companion assets/references, and current reviewer specialization is mode-based. | External bundle or role contract could be judged incorrectly. | Core readiness seam. |
| MAP-10 | `/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml` | Workflow catalog | Current real skill-role usage | 2026-04-01 | High | Proposal reviewer agents still use `skill_ref: proposal_review_triad`, `skill_role`, structured review prompts, and per-role output contracts. | Mode-mapping applicability could be overstated. | Grounds Appendix B and `A4`. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Frozen run truth | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunStartSnapshot.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift` | Persisted at run creation | 2026-04-01 | High | Current immutable owner model remains `Run` frozen fields plus `RunStartSnapshot`. | Frozen-owner alignment could be judged incorrectly. | Core architecture seam. |
| DATA-02 | Execution provenance | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift`, `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift` | Persisted per execution | 2026-04-01 | High | `skillSnapshotHash` is still the only live skill provenance field today. | Provenance delta could be misstated. | Core architecture seam. |
| DATA-03 | Catalog env placeholder validation | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/YAMLValidator.swift` | Validation | 2026-04-01 | Medium | Current env placeholder validation covers paths generally, but skill-specific validation still needs proposal-owned extension. | Skill-path validation assumptions could drift. | Relevant to Section 4.1 / 7. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Alignment Result | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Frozen-start truth owner | Current repo | 2026-04-01 | High | Current repo extends frozen-run truth through immutable `Run` fields plus `RunStartSnapshot`. | Proposal now matches this owner explicitly. | Architecture closure. |
| INT-02 | Operator inspection spine | Baseline + current repo | 2026-04-01 | High | Current operator inspection still lives in run reports, comparison, recovery, and artifact-inspector surfaces. | Proposal now extends these shell-owned surfaces instead of inventing `AgentInspectorView`. | UI / UX closure. |
| INT-03 | External skill package contract | Current repo | 2026-04-01 | High | Current external skills are Codex bundles rooted at `SKILL.md`. | Proposal now matches the current bundle contract explicitly. | Architecture closure. |
| INT-04 | Shared-skill role behavior | Current repo | 2026-04-01 | High | `proposal_review_triad` is still mode-based (`product-only`, `ux-only`, `ui-only`, `architecture-only`). | Proposal now maps the current reviewer roles to the real specialist-mode contract. | Architecture / behavior closure. |
| INT-05 | Current review prompts and contracts | Current repo | 2026-04-01 | High | Proposal reviewer agents in the example catalog still demand structured review output with numeric score and discipline-specific focus. | Proposal Appendix B remains consistent with current app-level task prompts. | Prompt-assembly grounding. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-01, MAP-04 | `AgentCatalog`, `RunPlanCompiler` | Resolution + compile entry is clearly defined. |
| Happy path | Specified | DOC-01, MAP-03, MAP-06, INT-03, INT-04, INT-05 | `ResolvedAgent`, `GooseSessionBridge`, external skill bundles, example agent prompts | Current proposal now fits repo reality on bundle and role/mode semantics. |
| Loading | Deferred intentionally | DOC-01 | N/A | Proposal remains about runtime resolution/injection, not loading UI states. |
| Empty | Partial | DOC-01, NAV-01 | `AgentCatalogView` | Empty catalog/report styling is not deeply specified, but not a blocker for proposal-readiness. |
| Validation error | Specified | DOC-01, MAP-02, NAV-02 | `YAMLValidator`, `PreflightService`, `PilotReadinessView` | Validation and blocking behavior are explicit. |
| Backend error | Partial | DOC-01, MAP-06 | `GooseSessionBridge` | Compile/preflight failure is clear; post-compile runtime-injection error handling remains mostly implementation detail. |
| Offline / degraded | Deferred intentionally | DOC-01, NAV-02 | `PilotReadinessView` | No special offline behavior is needed beyond preflight / run-start blocking for MVP. |
| Retry / recovery | Partial | DOC-01, NAV-02, NAV-03 | readiness + report shell | Recovery routing is sufficiently anchored to current shell, even though detailed runtime proof belongs to implementation audit. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | N/A | Not in scope for this proposal. |
| Rollback / cancellation | Partial | DOC-01, DOC-05 | report / comparison / recovery shell | Clone/resume truth is sufficiently anchored through immutable run-start ownership; detailed rollback UX is outside the core slice. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | Whole slice | Missing | Missing | 2026-04-01 | Medium | No blocker for proposal-readiness because the slice is bounded and implementation proof is deferred to audit. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | None specified | N/A | N/A | 2026-04-01 | Medium | No blocker for proposal-readiness; instrumentation can be evaluated during implementation if the slice expands. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-defined | resolution / injection / provenance / preflight / shell-owned UI smoke | Current repo still has no dedicated live skill-resolution slice | Proposal defines unit + integration + UI smoke coverage for all three skill types plus role-mapped specialization | 2026-04-01 | High | Test plan is explicit and now aligns with current owner model and bundle contract. |

## L. Current Repo Reality / Alignment
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Result |
|---|---|---|---|---|---|---|
| REAL-01 | External skill package structure | MVP external skills are Codex bundles rooted at `SKILL.md` | Current configured external skills are rooted at `SKILL.md` and include companion files under `assets/`, `references/`, `evals/`, and `agents/` | 2026-04-01 | High | Aligned |
| REAL-02 | Shared `proposal_review_triad` role behavior | Current reviewer roles map to explicit specialist modes | Current `proposal_review_triad` contract is mode-based (`product-only`, `ux-only`, `ui-only`, `architecture-only`) | 2026-04-01 | High | Aligned |
| REAL-03 | Frozen snapshot owner | Immutable `Run` + `RunStartSnapshot` remain the frozen owner | Current concrete owner model is immutable `Run` fields plus `RunStartSnapshot` | 2026-04-01 | High | Aligned |
| REAL-04 | Operator execution inspection | Existing shell-owned report / comparison / artifact surfaces are the visibility owners | Current shell still uses report/comparison/recovery/artifact routes and no standalone `AgentInspectorView` | 2026-04-01 | High | Aligned |
| REAL-05 | Provenance under size limits | MVP fail-closes instead of truncating executable content silently | Current repo still needs this slice implemented, but the proposal’s textual contract is now internally consistent | 2026-04-01 | High | Aligned |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-04 | Motivation remains strong and bounded. |
| Scope boundaries | Specified | DOC-01 | In/out-of-scope sections remain explicit. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, INT-01..05 | Proposal now matches current owner model and bundle contract. |
| Screen / surface definition | Specified | NAV-01..03, INT-02 | Catalog, readiness, and execution visibility are anchored to current shell owners. |
| Navigation / entry points | Specified | NAV-01..03 | Entry points are clear and grounded. |
| State handling | Partial | H matrix | Some degraded/runtime-heavy states remain intentionally deferred, but no blocker remains. |
| Data / API contract | Specified | DATA-01, DATA-02, REAL-03, REAL-05 | Raw and injected truth ownership is now explicit. |
| Persistence / caching | Specified | DATA-01, DATA-02 | Frozen and per-execution provenance contracts are now coherent for MVP. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Explicitly out of scope. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | No blocker for this proposal-sized slice. |
| Analytics / instrumentation | Partial | METRIC-01 | Not blocking readiness. |
| Testing strategy | Specified | TEST-01 | Coverage plan is explicit and aligned. |
| Dependencies / integration points | Specified | MAP-01..10, INT-01..05 | Current repo seams and proposal seams now match. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Current `.codex/skills/*` bundle shape remains the intended external skill source for MVP.
- ASSUMP-02: Current example proposal-review prompts continue to represent the intended structured review contract.
- QUESTION-01: If future proposals promote companion bundle files into executable truth, should the runtime flatten them at compile time or preserve typed references?
- BLOCKER-01: None

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | QUESTION-01 | If companion bundle files ever become executable truth, what is the best contract for preserving provenance without over-flattening the skill bundle? | Current local evidence is sufficient for MVP, but not for a future richer skill-runtime model. | Low |
