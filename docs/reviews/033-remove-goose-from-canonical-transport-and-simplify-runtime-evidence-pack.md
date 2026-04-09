# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-09 | High | `P033` removes Goose from the canonical transport path, keeps Goose only as optional compatibility adapter, and also removes `mcp_profile` / `mcp_server_registry` from the canonical repo model. | The review could miss the actual scope and over- or under-call readiness. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-09 | High | The prerequisite second-wave ACP slice is still `Overall Conformance = Not Implemented`, `Overall Readiness = Not Ready`, because adapter-aware MCP ownership and successful proof paths for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp` are still open. | `P033` could be judged as ready even though its required predecessor is not finished. | Dependency gate. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | Review should start from stable reference docs and only refresh the affected runtime/provider slice. | Review could redo unnecessary archaeology or miss subsystem contracts. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current HEAD baseline includes live Goose-backed execution, ACP-shaped runtime transport with Goose compatibility transport, per-agent MCP truth, provider/settings platform, operator shell, and Goose remediation journey. | Proposal could be reviewed as if those baseline owners were not already implemented. | Current host-system truth. |
| DOC-05 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Stable transport reference still treats Goose as implemented compatibility adapter and part of the current factory/default path, not something already retired from canonical runtime. | Proposal could skip the migration delta from current transport truth. | Runtime baseline. |
| DOC-06 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-09 | High | The current canonical MCP model is repo-owned through `mcp_profile`, `mcp_policy`, `mcp_server_registry`, and `mcp_profiles`, with frozen requested/predicted/actual/denied truth. | Proposal could remove a core repo contract without specifying migration. | MCP ownership baseline. |
| DOC-07 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Provider setup/readiness is already a product surface with `ProviderSettingsView`, `PilotReadinessView`, diagnostics, troubleshooting, and machine-local configuration owners. | Proposal could under-specify operator-facing migration work. | Settings/readiness baseline. |
| DOC-08 | `docs/reference/workflow-execution-engine.md` | 2026-04-09 | High | Current compile/runtime model freezes `mcp_profile` on resolved agents and still documents a Goose-backed live executor path. | Proposal could understate how deep the MCP and Goose assumptions run in current engine truth. | Engine baseline. |
| DOC-09 | `docs/reference/operator-experience.md` | 2026-04-09 | High | Operator runtime provenance currently exposes only `Fixture / verified baseline`, `Goose server / trust pending`, and `Goose server / verified`. | Proposal could demote Goose without defining a replacement runtime-trust vocabulary. | Shell/runtime-trust baseline. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level baseline posture and stable-reference map | 2026-04-09 | High | Still fresh for review setup. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current implemented product boundaries and stable subsystem map | 2026-04-09 | High | Fresh overall, but affected transport/MCP slice needed targeted code refresh. | Current system map. |
| BASE-03 | stable runtime/provider/MCP reference docs | Partially refreshed | transport, provider, MCP, operator shell | 2026-04-09 | High | The transport slice is partially stale against current second-wave runtime work and Goose-first UI copy, so direct code refresh was required. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-09 | High | No `033...review/integration-context.md` exists. Not blocking because the stable refs plus targeted code refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Partially refreshed | prerequisite transport/provider proof status | 2026-04-09 | High | Fresh enough to establish dependency readiness. | Dependency check. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - remove Goose as canonical runtime transport path
  - ACP-first dispatch simplification
  - Goose compatibility-only packaging
  - default runtime migration away from Goose
  - post-Goose MCP ownership model
  - proof/evidence/doc updates for the simplified runtime
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from system-level settings
  - weakening execution/recovery/report truth
- Deferred intentionally:
  - none explicitly beyond compatibility retention
- Assumptions:
  - `P033` is judged as a delta over current stable refs and current HEAD
  - user requested proposal readiness only, without product overlay or web research
- Open questions:
  - does `P033` require `P030` to be fully implemented before any work starts, or does it expect overlap?
  - what exact schema replaces `agent.mcp_profile` / `mcp_profiles` / `mcp_server_registry` in repo-owned YAML?
  - which operator surfaces remain Goose-specific compatibility entry points after the migration?
- Blockers:
  - dependency on `P030` is not actually satisfied
  - MCP/YAML migration contract is not specified enough to implement safely

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Targeted refresh | 2026-04-09 | High | Settings currently tells operators to “Use Goose-backed setup first for Codex and Claude,” includes a `Managed Goose Server` section, and opens Goose Assistant from provider rows. | Proposal could miss a large part of the operator-facing migration. | Canonical provider setup surface. |
| NAV-02 | `FirstRunSetupWizard` | Targeted refresh | 2026-04-09 | High | The first-run journey currently offers `Add Codex via Goose`, `Add Claude via Goose`, states “Codex and Claude are Goose-first in the app,” and includes a managed Goose server section. | Proposal could claim onboarding is updated without owning the actual onboarding flow. | Primary onboarding journey. |
| NAV-03 | `PilotReadinessView` + runtime trust shell | Targeted refresh + baseline | 2026-04-09 | High | Pilot readiness still exposes Goose base URL, managed Goose server state, and Goose Assistant entry points; operator runtime provenance docs are Goose-only. | Proposal could demote Goose without defining replacement operator trust and readiness states. | Readiness and trust surfaces. |
| NAV-04 | `IdeaListView` / Start Run live mode | Targeted refresh | 2026-04-09 | High | Live mode still says “Uses configured Goose-backed execution,” “Live workflows require an available Goose runtime,” and “Executor: Goose-backed live execution.” | Proposal could leave run-start and recovery copy inconsistent with the new canonical runtime model. | Run-start/operator journey. |
| NAV-05 | `GooseProviderConnectionAssistantView` | Targeted refresh | 2026-04-09 | High | The repo has a dedicated Goose Connection Assistant with verification, remediation, and explicit Goose-backed framing. | Proposal could keep Goose compatibility but fail to decide whether this surface stays, narrows, or moves. | Compatibility-only UI boundary. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/RuntimeTransport.swift` | Runtime transport | canonical transport protocol and session request shape | 2026-04-09 | High | The canonical seam is already ACP-shaped and already materializes ACP-native `mcpServers` in `RuntimeSessionRequest`. | Proposal could overclaim transport simplification that already partially landed. | Existing transport truth. |
| MAP-02 | `Chainworks Forge/Engine/ExecutionService.swift` | Runtime selection | live executor construction and Goose transport resolution | 2026-04-09 | High | Current live executor still constructs and caches Goose transport explicitly via `resolveGooseTransport`, including `GooseTransport`, `GooseServerTransport`, and `FixtureGooseTransport`. | Proposal must specify exactly what changes in core vs compatibility code. | Main transport migration seam. |
| MAP-03 | `Chainworks Forge/Engine/GooseAgentExecutor.swift` and `Chainworks Forge/Engine/GooseSessionBridge.swift` | Execution engine | runtime execution, cancellation, MCP resolution, session bridge | 2026-04-09 | High | The executor/session bridge are transport-neutral in parts, but still carry Goose-named owners, Goose-only default registry provider, and Goose transport references for cancellation. | Proposal must define the target owner matrix, not just say “compatibility-only modules.” | Core orchestration boundary. |
| MAP-04 | `Chainworks Forge/DSL/AgentCatalog.swift` and `Chainworks Forge/DSL/YAMLValidator.swift` | DSL / validation | current canonical MCP/YAML ownership | 2026-04-09 | High | `AgentCatalog` still has `mcpPolicy`, `mcpServerRegistry`, `mcpProfiles`, and `AgentDefinition.mcpProfile`; validation treats them as authoritative. | Removing them without a replacement schema would break the current catalog contract. | Main data-model blocker. |
| MAP-05 | `Chainworks Forge/Engine/RunPlanCompiler.swift` | Compile-time freezing | freezes `agent.mcpProfile` into resolved agents | 2026-04-09 | High | `RunPlanCompiler` currently sets `ResolvedAgent.mcpProfileID` from `agentDef.mcpProfile`. | Proposal must specify migration for compile-time frozen MCP intent. | Freeze-truth boundary. |
| MAP-06 | `Chainworks Forge/Models/Run.swift`, `Chainworks Forge/Models/AgentExecution.swift` | Persistence | frozen MCP policy and per-execution MCP truth | 2026-04-09 | High | `Run.resolvedMCPPoliciesJSON` and `AgentExecution.mcpProfileID` + MCP arrays are persisted fields, not temporary glue. | Proposal must define compatibility and migration for persisted truth. | Persistence/report boundary. |
| MAP-07 | `Chainworks Forge/Engine/RunReportBuilder.swift` and `Chainworks Forge/Engine/RunComparisonService.swift` | Reports / comparison | operator-visible MCP truth readers | 2026-04-09 | High | Reports/comparison still read and render `mcpProfileID`, requested/predicted/actual/denied MCP truth from frozen and execution records. | Proposal must decide how operator-visible truth survives the MCP-model rewrite. | Operator evidence boundary. |
| MAP-08 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift` and `Chainworks Forge/Engine/PreflightService.swift` | MCP policy / readiness | runtime validation and machine-local realization | 2026-04-09 | High | Current MCP policy resolution depends on catalog-owned `mcp_profile` / `mcp_server_registry`, while machine-local registry handling remains partly Goose-centric. | Proposal must specify the new repo/local ownership split, not just its direction. | Core MCP migration seam. |
| MAP-09 | `scripts/test-gate.sh` and `docs/reference/test-gates.md` | Verification | canonical test-gate ownership | 2026-04-09 | High | There is no `proposal-033` gate yet, even though `P033` proposes a distinct post-Goose proof slice. | Proposal testing strategy is incomplete. | Verification blocker. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Canonical MCP YAML model | `AgentCatalog.swift`, `YAMLValidator.swift`, `per-agent-mcp-policy-and-runtime-validation.md` | Repo-owned schema | 2026-04-09 | High | Current repo truth makes `mcp_profile`, `mcp_profiles`, and `mcp_server_registry` first-class DSL inputs with validation and policy semantics. | `P033` could describe a target state without any safe migration path from current canonical data. | Main architecture blocker. |
| DATA-02 | Frozen run/report MCP truth | `RunPlanCompiler.swift`, `Run.swift`, `AgentExecution.swift`, `RunReportBuilder.swift`, `RunComparisonService.swift` | Persisted truth | 2026-04-09 | High | MCP intent and settlement are frozen into run/execution/report data, not reconstructed on the fly. | Removing MCP repo owners without a compatibility story would break report, comparison, and auditability guarantees. | Persistence/report blocker. |
| DATA-03 | Machine-local Goose compatibility configuration | `ExecutionService.swift`, `ProviderSettingsView.swift`, `FirstRunSetupWizard.swift`, `provider-platform.md` | Local config + onboarding | 2026-04-09 | High | Goose compatibility is currently visible in setup, readiness, and live executor construction. | Proposal could underestimate how much local config and onboarding logic must be migrated. | Operator migration boundary. |
| DATA-04 | Dependency proof lane | `030...IMPLEMENTATION_AUDIT_R4.md` | Proposal dependency | 2026-04-09 | High | The required second-wave ACP proof lane is not yet closed. | Implementing `P033` now would retire the canonical Goose path before the replacement path is fully proven. | Dependency gate. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/onboarding shell | Baseline + current repo | 2026-04-09 | High | Provider settings, first-run wizard, readiness, and Goose Assistant currently encode Goose-first setup and trust messaging. | Proposal does not yet own a surface-by-surface migration contract. | UI/UX blocker. |
| INT-02 | Live runtime dispatch | Current repo | 2026-04-09 | High | Execution still builds a Goose transport path in core runtime construction, even while ACP adapters coexist. | Proposal must define what stays in core, what becomes compatibility-only, and what gets renamed or retained. | Core runtime blocker. |
| INT-03 | MCP ownership split | Baseline + current repo | 2026-04-09 | High | Current system divides MCP truth between repo-owned policy and machine-local realization. `P033` removes the repo-owned half without defining a new schema or migration. | Main architecture blocker. | Canonical contract blocker. |
| INT-04 | Runtime provenance shell | Baseline + current repo | 2026-04-09 | High | Operator runtime provenance still exposes only Fixture and Goose server trust states. | Proposal demotes Goose but does not define the post-Goose operator trust taxonomy. | UX/trust blocker. |
| INT-05 | Proof-gate ownership | Current repo | 2026-04-09 | High | Existing gates cover proposal-006/012/014/030/035 and broader fast/full lanes, but nothing yet owns post-Goose canonical proof. | Proposal verification is not operationally grounded. | Readiness blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Partial | DOC-01, NAV-01, NAV-02, NAV-03 | Settings, first-run wizard, run start | Proposal states the goal but does not name the actual entry surfaces that must migrate. |
| Happy path | Partial | DOC-01, MAP-02, MAP-03, INT-02 | runtime dispatch, compatibility adapter | ACP-first happy path is stated, but the exact dispatch and compatibility-owner matrix is missing. |
| Loading | Missing | NAV-01, NAV-02, NAV-03 | readiness refresh, setup verification | No loading/transition states are specified for ACP-default vs Goose-compatibility migration. |
| Empty | Partial | DOC-01, NAV-01 | provider setup surfaces | Proposal keeps Goose compatibility but does not define empty/default state when only compatibility path exists or ACP is unavailable. |
| Validation error | Partial | DOC-01, MAP-04, MAP-08 | YAML validation, preflight | Proposal says preflight compares backend-declared intent to machine-local capability, but does not define the replacement schema or exact failures. |
| Backend error | Partial | DOC-01, MAP-02, MAP-03, MAP-08 | transport/runtime failures | Risks name regressions, but no explicit operator or retry semantics are defined for ACP-default failure vs Goose compatibility fallback. |
| Offline / degraded | Partial | DOC-01, DOC-07, NAV-02, NAV-05 | provider troubleshooting, readiness | Proposal retains Goose tooling but does not define degraded-mode states across shell surfaces. |
| Retry / recovery | Deferred intentionally | DOC-01, DOC-04, DOC-08 | execution truth/recovery refs | Proposal promises no truth-regression, but does not introduce a new recovery model. |
| Auth / permission expiry | Missing | NAV-01, NAV-02, DATA-03 | provider setup/auth readiness | No explicit auth/credential migration or expiry handling is specified for the new ACP-default world. |
| Rollback / cancellation | Partial | DOC-01, DATA-03, DATA-04 | compatibility fallback, provider enablement | Proposal keeps Goose compatibility as fallback, but does not define the rollback trigger or operator-visible hold/rollback path. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Current provider `isEnabled` rollout gates and Goose compatibility presence | provider/runtime selection | current repo can stage providers via `ConfiguredProvider.isEnabled`, but `P033` does not specify the cutover flag/selection model for Goose retirement | proposal implies compatibility fallback, but no explicit rollback gate or hold criteria are defined | 2026-04-09 | Medium | Rollout intent exists, but the actual post-Goose cutover gate is missing from the proposal. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | proposal-specific proof gate | readiness proof | `P033` rollout/proof sequence | 2026-04-09 | High | No `proposal-033` gate or equivalent proof owner exists yet, even though the proposal defines a new canonical transport shape. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite implementation audit | second-wave ACP readiness | `P030` audit proves the prerequisite is still not fully implemented | `P033` must require `P030` Green implementation readiness first | 2026-04-09 | High | The prerequisite proof lane is still red. |
| TEST-02 | focused proposal gate | post-Goose canonical transport | no `proposal-033` gate exists in `scripts/test-gate.sh` or `docs/reference/test-gates.md` | add an explicit `proposal-033` proof gate or equivalent canonical lane | 2026-04-09 | High | Proposal verification is not operationally grounded. |
| TEST-03 | unit/integration/report coverage | MCP schema and report migration | current coverage assumes `mcp_profile` / `mcp_server_registry` / frozen MCP policy truth still exist | define migration tests for compiler, run snapshot, preflight, report, comparison, and compatibility fallback | 2026-04-09 | High | The proposal removes a large data model without specifying the required regression coverage. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | prerequisite second-wave ACP slice | `P033` starts after second-wave ACP providers are implemented and proven | `P030` is still `Overall Conformance = Not Implemented`, `Overall Readiness = Not Ready` | 2026-04-09 | High | `P033` currently sits on an unmet dependency. |
| REAL-02 | canonical MCP repo model | `P033` removes `mcp_profile` and `mcp_server_registry` from canonical repo YAML | current DSL, validator, compiler, persistence, reports, and comparison still use them as canonical truth | 2026-04-09 | High | The proposal lacks a safe migration contract. |
| REAL-03 | operator shell and onboarding | `P033` keeps Goose only as optional compatibility adapter and says docs/onboarding should stop implying Goose is canonical | current shell is Goose-first across Settings, First Run Wizard, Pilot Readiness, Goose Assistant, Start Run, and runtime provenance docs | 2026-04-09 | High | Proposal under-specifies operator-surface migration. |
| REAL-04 | core runtime dispatch | `P033` removes Goose-shaped canonical assumptions from core orchestration | current core still explicitly resolves Goose transport, keeps Goose transport for cancellation, and defaults runtime registry access to Goose | 2026-04-09 | High | Proposal needs a stronger core-vs-compatibility owner matrix. |
| REAL-05 | verification ownership | `P033` says focused proof gates will validate the post-Goose transport shape | there is no `proposal-033` gate or equivalent proof lane yet | 2026-04-09 | High | Testing strategy is incomplete. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | The simplification goal is clear. |
| Scope boundaries | Partial | DOC-01, REAL-03, REAL-04 | Compatibility retention is named, but surface and module ownership are not. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05, DOC-06, DOC-07, DOC-08, DOC-09 | Stable local evidence is sufficient. |
| Screen / surface definition | Missing | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05 | Proposal does not enumerate the operator surfaces that must change. |
| Navigation / entry points | Partial | NAV-01, NAV-02, NAV-03 | Entry surfaces are inferable from repo reality, not locked in the proposal. |
| State handling | Partial | H table | Multiple migration states remain unspecified. |
| Data / API contract | Partial | DATA-01, DATA-02, MAP-04, MAP-05, MAP-06, MAP-07 | Direction is stated, but replacement schema and migration are missing. |
| Persistence / caching | Partial | DATA-02 | Proposal promises unchanged truth semantics, but does not describe migration for frozen MCP truth fields. |
| Permissions / auth expiry | Missing | DATA-03, H | No explicit credential/expiry migration path is defined. |
| Feature flags / rollout / rollback | Partial | FLAG-01, DATA-04 | Compatibility fallback is named, but cutover gate and rollback policy are not. |
| Analytics / instrumentation | Missing | METRIC-01 | No new proof/instrumentation ownership is defined for the slice. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Proof intent exists, but no concrete `P033` verification contract exists yet. |
| Dependencies / integration points | Contradicted by repo | DOC-02, REAL-01, REAL-02, REAL-03, REAL-04 | The proposal depends on prerequisite and integration surfaces that are not yet ready or not yet specified. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended as a delta over current stable refs and current code, not a greenfield transport rewrite.
- ASSUMP-02: the proposal expects `P030` to reach real implementation-ready completion before `P033` begins.
- QUESTION-01: what exact YAML schema replaces `agent.mcp_profile`, `mcp_profiles`, and `mcp_server_registry`?
- QUESTION-02: which operator surfaces keep Goose compatibility affordances, and which must become ACP-first?
- QUESTION-03: what replaces the current Goose-only runtime provenance states in reports/comparison/shell?
- BLOCKER-01: prerequisite `P030` proof/readiness is not yet closed.
- BLOCKER-02: canonical MCP/YAML migration contract is not specified.
- BLOCKER-03: operator-surface migration and proof-gate ownership are not specified enough to implement safely.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
