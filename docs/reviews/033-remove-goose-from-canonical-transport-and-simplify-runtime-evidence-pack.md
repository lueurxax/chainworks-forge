# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-09 | High | `P033` now includes a hard `P030` prerequisite gate, a three-phase MCP migration, a Goose core/compatibility split, an operator-surface migration table, and a post-Goose trust vocabulary. | The review could call already-fixed gaps as still open, or miss the remaining narrower blockers. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-09 | High | The prerequisite second-wave ACP slice is still `Overall Conformance = Not Implemented`, `Overall Readiness = Not Ready`. | `P033` could be judged as immediately startable even though its explicit prerequisite is still red. | Dependency gate. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | Review should start from stable reference docs and only refresh the affected runtime/provider slice. | Review could redo unnecessary archaeology or miss subsystem contracts. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current HEAD baseline still includes live Goose-backed execution, ACP-shaped runtime transport with Goose compatibility, per-agent MCP truth, and Goose remediation/operator surfaces. | Proposal could be reviewed as if those current owners were already retired. | Current host-system truth. |
| DOC-05 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Stable transport truth is ACP-shaped but still keeps Goose compatibility transport and Goose-owned runtime paths in the current factory. | Proposal could overstate how much simplification is already done. | Runtime baseline. |
| DOC-06 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-09 | High | The current MCP contract is richer than a flat server list: it includes repo-owned profiles, required vs optional extensions, fallback policy, runtime mapping, and frozen requested/predicted/actual/denied truth. | Proposal could collapse MCP truth into a thinner field and silently lose current semantics. | MCP ownership baseline. |
| DOC-07 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Provider settings/readiness and Goose remediation are already stable product surfaces with explicit operator ownership. | Proposal could under-specify surface migration work. | Settings/readiness baseline. |
| DOC-08 | `docs/reference/workflow-execution-engine.md` | 2026-04-09 | High | Current compile/runtime model still freezes `mcp_profile` on agents and documents Goose-backed live execution paths. | Proposal could understate how deep current Goose/MCP assumptions run. | Engine baseline. |
| DOC-09 | `docs/reference/operator-experience.md` | 2026-04-09 | High | Operator runtime provenance is still expressed in Goose-centric vocabulary. | Proposal could rename trust states without a compatibility story for existing shell/report readers. | Operator trust baseline. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level baseline posture and stable-reference map | 2026-04-09 | High | Still fresh for review setup. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current implemented product boundaries and stable subsystem map | 2026-04-09 | High | Fresh overall, but the affected transport/MCP/operator slice required direct code refresh. | Current system map. |
| BASE-03 | stable runtime/provider/MCP reference docs | Partially refreshed | transport, provider, MCP, operator shell | 2026-04-09 | High | The stable refs are still valid, but the proposal needed a narrow reality check for current Goose owners, legacy trust readers, and richer MCP truth. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-09 | High | No `033...review/integration-context.md` exists. Not blocking because stable refs plus targeted code refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Reused | prerequisite transport/provider proof status | 2026-04-09 | High | Fresh enough to establish dependency readiness. | Dependency check. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - remove Goose as canonical runtime transport path
  - ACP-first dispatch simplification
  - Goose compatibility-only packaging
  - default runtime migration away from Goose
  - phased MCP ownership migration
  - operator-surface migration and trust vocabulary
  - proof/evidence/doc updates for the simplified runtime
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from all system-level settings
  - weakening execution/recovery/report truth
- Deferred intentionally:
  - full Goose removal from operator workflows
- Assumptions:
  - `P033` is judged as a delta over current stable refs and current HEAD
  - user requested proposal readiness only, without product overlay or web research
- Open questions:
  - what is the exact shape of `backend_profile.mcp_intent`?
  - how do legacy `server_unverified` / `server_verified` runs map into the new trust vocabulary?
  - which Goose-specific fixtures, assistants, and preview/test surfaces remain compatibility-only after the migration?
- Blockers:
  - the MCP migration contract is still too thin for safe implementation
  - the Goose owner/surface inventory is not yet exhaustive
  - runtime-trust migration/back-compat is not specified enough for historical runs and readers

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Targeted refresh | 2026-04-09 | High | Settings still tells operators to “Use Goose-backed setup first for Codex and Claude,” includes `Managed Goose Server`, and opens Goose Assistant from provider rows. | Proposal could miss the most obvious setup migration surface. | Canonical provider setup surface. |
| NAV-02 | `FirstRunSetupWizard` | Targeted refresh | 2026-04-09 | High | First-run still offers `Add Codex via Goose`, `Add Claude via Goose`, states “Codex and Claude are Goose-first in the app,” and includes managed Goose server controls. | Proposal could claim onboarding coverage while missing the actual journey. | Primary onboarding journey. |
| NAV-03 | `PilotReadinessView` | Targeted refresh | 2026-04-09 | High | Pilot readiness still exposes Goose base URL, managed Goose server state, and Goose Assistant entry points. | Proposal surface coverage could remain partial if this surface is not named explicitly. | Readiness journey. |
| NAV-04 | `IdeaListView` / Start Run live mode | Targeted refresh | 2026-04-09 | High | Live mode still says “Uses configured Goose-backed execution” and “Live workflows require an available Goose runtime.” | Proposal could leave run-start and live-mode messaging inconsistent with ACP-first reality. | Run-start/operator journey. |
| NAV-05 | `GooseProviderConnectionAssistantView` | Targeted refresh | 2026-04-09 | High | The repo has a dedicated Goose assistant with its own verification journey, remediation copy, and return paths into setup/readiness. | Proposal could keep Goose compatibility but fail to decide whether this surface stays, narrows, or moves. | Compatibility-only UI boundary. |
| NAV-06 | `RunsHomeView` runtime provenance badge | Targeted refresh | 2026-04-09 | High | The main shell still renders `server_unverified` / `server_verified` as “Goose server / trust pending” and “Goose server / verified.” | Proposal could rename trust states without defining reader compatibility for historical runs. | Operator trust/history surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/RuntimeTransport.swift` | Runtime transport | canonical transport protocol and session request shape | 2026-04-09 | High | The canonical seam is already ACP-shaped and already supports ACP-native `mcpServers` in `RuntimeSessionRequest`. | Proposal could overclaim transport simplification that already partially landed. | Existing transport truth. |
| MAP-02 | `Chainworks Forge/Engine/ExecutionService.swift` | Runtime selection | live executor construction and Goose transport resolution | 2026-04-09 | High | Current live executor still constructs and caches Goose transport explicitly via `resolveGooseTransport`, including Goose server, bespoke Goose, and fixture transports. | Proposal must classify more than just the top-level factory. | Main transport migration seam. |
| MAP-03 | `Chainworks Forge/Engine/GooseAgentExecutor.swift` and `Chainworks Forge/Engine/GooseSessionBridge.swift` | Execution engine | runtime execution, cancellation, session bridge, MCP resolution | 2026-04-09 | High | These types are partly transport-neutral now, but still carry Goose-named owners, Goose-only defaults, and Goose-specific cancellation helpers. | Proposal must define the full core/compatibility split, not just a rename intent. | Core orchestration boundary. |
| MAP-04 | `Chainworks Forge/DSL/AgentCatalog.swift` and `Chainworks Forge/DSL/YAMLValidator.swift` | DSL / validation | canonical MCP/YAML ownership | 2026-04-09 | High | The current catalog contract includes `mcp_policy`, `mcp_server_registry`, `mcp_profiles`, `agent.mcp_profile`, runtime authority, required/optional extension sets, and fallback semantics. | A thin `mcp_intent` field can lose important current truth. | Main data-model blocker. |
| MAP-05 | `Chainworks Forge/Engine/RunPlanCompiler.swift`, `Chainworks Forge/Models/Run.swift`, `Chainworks Forge/Models/AgentExecution.swift`, `Chainworks Forge/Engine/RunReportBuilder.swift`, `Chainworks Forge/Views/RunComparisonView.swift` | Compile/persist/report | frozen MCP truth and operator-visible readers | 2026-04-09 | High | Current compile-time and report surfaces still freeze and read `mcpProfileID`, requested/predicted/actual/denied MCP truth, and legacy trust vocabulary. | Proposal must specify compatibility for both frozen runs and historical readers. | Freeze-truth boundary. |
| MAP-06 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift` | MCP policy / readiness | runtime validation and machine-local realization | 2026-04-09 | High | Current resolver depends on required vs optional extensions, fallback policy, runtime namespace mapping, and machine-local availability. | Proposal must specify how these semantics survive the migration. | MCP realization boundary. |
| MAP-07 | `Chainworks Forge/Providers/GooseProviderConnectionAssistant.swift` and `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift` | Provider remediation | dedicated Goose verification journey | 2026-04-09 | High | Goose compatibility already has a distinct remediation model, not just incidental copy. | Proposal must explicitly decide this surface’s fate. | Compatibility UX boundary. |
| MAP-08 | `Chainworks Forge/Engine/FixtureGooseTransport.swift` and `Chainworks Forge/Support/PreviewSupport.swift` | Fixtures / previews | proof fixtures and preview trust examples | 2026-04-09 | High | Goose naming and legacy trust vocabulary also exist in fixture and preview layers. | Proposal can leave a long tail of “canonical Goose” assumptions if fixtures are not classified. | Proof/test boundary. |
| MAP-09 | `scripts/test-gate.sh` and `docs/reference/test-gates.md` | Verification | focused proof-lane ownership | 2026-04-09 | High | There is no `proposal-033` gate yet; the proposal owns defining it. | The verification plan can stay underspecified if exact suites and evidence outputs are not named. | Verification boundary. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Canonical MCP schema | `AgentCatalog.swift`, `YAMLValidator.swift`, `per-agent-mcp-policy-and-runtime-validation.md` | Repo-owned schema | 2026-04-09 | High | Current repo truth distinguishes required vs optional extensions, fallback behavior, runtime authority, and per-runtime server mappings. | Proposal could replace a rich contract with an underspecified field and lose behavior. | Main architecture blocker. |
| DATA-02 | Frozen MCP and trust truth | `RunPlanCompiler.swift`, `Run.swift`, `AgentExecution.swift`, `RunReportBuilder.swift`, `RunComparisonView.swift` | Persisted truth | 2026-04-09 | High | MCP settlement and `runtimeTrustLevel` are persisted and later rendered by reports/comparison/shell surfaces. | Proposal could change vocabulary or ownership without a compatibility story for historical runs. | Persistence/report blocker. |
| DATA-03 | Machine-local Goose compatibility configuration | `ExecutionService.swift`, `ProviderSettingsView.swift`, `FirstRunSetupWizard.swift`, `PilotReadinessView.swift`, `provider-platform.md` | Local config + onboarding | 2026-04-09 | High | Goose compatibility is visible in setup, readiness, remediation, and live executor construction. | Proposal could underestimate how much local config and UX must migrate. | Operator migration boundary. |
| DATA-04 | Dependency proof lane | `030...IMPLEMENTATION_AUDIT_R4.md` | Proposal dependency | 2026-04-09 | High | The required second-wave ACP proof lane is still not closed. | Implementation cannot start yet, even though the proposal now fail-closes this dependency correctly. | External hold. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/onboarding shell | Baseline + current repo | 2026-04-09 | High | Provider settings, first-run wizard, readiness, and Goose Assistant currently encode Goose-first setup and remediation. | Proposal migration table is directionally correct but not yet exhaustive. | UI/UX blocker. |
| INT-02 | Live runtime dispatch | Current repo | 2026-04-09 | High | Execution still builds a Goose transport path in core runtime construction, even while ACP adapters coexist. | Proposal must own the full core-vs-compatibility split, including fixtures. | Core runtime blocker. |
| INT-03 | MCP ownership split | Baseline + current repo | 2026-04-09 | High | Current system divides MCP truth between repo-owned policy and machine-local realization, with richer semantics than “required MCP servers.” | Proposal’s migration direction is right but not yet precise enough. | Canonical contract blocker. |
| INT-04 | Runtime provenance readers | Baseline + current repo | 2026-04-09 | High | Shell/report/preview surfaces still encode legacy `server_unverified` / `server_verified` trust values. | Proposal defines new trust states but not reader/backfill compatibility. | UX/trust blocker. |
| INT-05 | Proof-gate ownership | Current repo + proposal | 2026-04-09 | High | The proposal now owns a dedicated `proposal-033` gate, but exact suite/evidence composition is still not named. | Verification can stay ambiguous if gate ownership remains only conceptual. | Readiness blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Partial | DOC-01, NAV-01, NAV-02, NAV-03 | Settings, first-run wizard, readiness | Proposal now names some entry surfaces, but not all current Goose-first entry points. |
| Happy path | Partial | DOC-01, MAP-02, MAP-03, INT-02 | runtime dispatch, compatibility adapter | ACP-first happy path is stated, but the full owner matrix still is not exhaustive. |
| Loading | Partial | DOC-01, NAV-03, NAV-05 | readiness refresh, assistant verification | Proposal direction is present, but loading/verification states are not mapped per surface. |
| Empty | Partial | DOC-01, NAV-01, NAV-02 | provider setup surfaces | No explicit empty/default state is defined when ACP is unavailable but Goose compatibility still exists. |
| Validation error | Partial | DOC-01, MAP-04, MAP-06 | YAML validation, preflight | Proposal says preflight validates `mcp_intent`, but not what exact validation semantics replace required/optional/fallback behavior. |
| Backend error | Partial | DOC-01, MAP-02, MAP-03, MAP-07 | transport/runtime failures | Proposal names compatibility fallback but not the precise operator or retry semantics per failure class. |
| Offline / degraded | Partial | DOC-01, NAV-03, NAV-05, DATA-03 | readiness, assistant, provider troubleshooting | The compatibility journey exists in repo reality, but proposal does not fully map degraded ACP vs Goose fallback states. |
| Retry / recovery | Deferred intentionally | DOC-01, DOC-04 | execution truth/recovery refs | Proposal promises no truth regression and leaves recovery model intact. This is acceptable if trust migration is made explicit. |
| Auth / permission expiry | Missing | NAV-01, NAV-02, DATA-03 | provider setup/auth readiness | No explicit auth/credential-expiry migration is defined for the new ACP-default trust model. |
| Rollback / cancellation | Partial | DOC-01, DATA-03, DATA-04 | compatibility fallback, provider enablement | Proposal keeps compatibility fallback, but rollback/hold semantics are still high-level. |
| Historical-read compatibility | Missing | DOC-01, DOC-09, MAP-05, INT-04 | reports, comparison, RunsHome, previews | Proposal defines new trust states but does not define how old stored values keep rendering correctly. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Current provider `isEnabled` rollout gates and Goose compatibility presence | provider/runtime selection | proposal shifts defaults away from Goose and keeps compatibility fallback | rollback path is implicit through compatibility retention, but exact cutover flag/hold criteria are still not explicit | 2026-04-09 | Medium | Direction exists, but operational cutover details remain partial. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | readiness proof | rollout/proof sequence in `P033` | 2026-04-09 | High | Proposal now owns the gate, but exact suites and artifact expectations are still unnamed. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite implementation audit | second-wave ACP readiness | `P030` audit proves the prerequisite is still not fully implemented | `P033` correctly hard-gates on `P030` Green readiness | 2026-04-09 | High | External hold remains, but no longer a proposal contradiction. |
| TEST-02 | focused proposal gate | post-Goose canonical transport | no `proposal-033` gate exists yet | add `proposal-033` with named suites for MCP migration, ACP-default dispatch, Goose compatibility, and trust readers | 2026-04-09 | High | Verification plan still needs more operational specificity. |
| TEST-03 | unit/integration/report coverage | MCP schema, trust migration, compatibility readers | current coverage assumes legacy MCP/trust structures still exist | define migration tests for compiler, run snapshot, report, comparison, previews, and trust readers | 2026-04-09 | High | Trust/back-compat proof is currently missing from the proposal text. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | canonical MCP repo model | `P033` introduces `backend_profile.mcp_intent` for Phase 1 and later removes the old MCP structures | current DSL/runtime/report truth is richer than “required MCP servers” and depends on required/optional/fallback/runtime-mapping semantics | 2026-04-09 | High | The proposal direction is valid, but the new schema contract is still underspecified. |
| REAL-02 | Goose owner/surface inventory | `P033` says every Goose-touching file and UI surface is classified | current repo still has additional Goose owners and surfaces not named in the proposal: fixture transport, provider assistant service/view, pilot readiness, and trust renderers | 2026-04-09 | High | Inventory is still incomplete. |
| REAL-03 | trust vocabulary and readers | `P033` introduces `runtime_unverified`, `runtime_verified`, and `compatibility_fallback` | current persisted runs, previews, and shell/report readers still use `server_unverified` / `server_verified` | 2026-04-09 | High | Proposal needs explicit migration/back-compat rules. |
| REAL-04 | prerequisite second-wave ACP slice | `P033` cannot begin until `P030` is green | `P030` is still red today | 2026-04-09 | High | Implementation is correctly blocked by an explicit prerequisite gate. |
| REAL-05 | verification ownership | `P033` says `proposal-033` gate will enforce the migration | the gate does not exist yet, and proposal text does not yet name its exact suite/evidence makeup | 2026-04-09 | High | Verification is directionally owned but still operationally partial. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | The simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | In-scope vs out-of-scope is explicit. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05, DOC-06, DOC-07, DOC-08, DOC-09 | Stable local evidence is sufficient. |
| Screen / surface definition | Partial | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05, NAV-06, REAL-02 | The proposal now names some surfaces, but not the full current surface set. |
| Navigation / entry points | Partial | NAV-01, NAV-02, NAV-03, NAV-04 | Entry surfaces are partly locked, not exhaustive. |
| State handling | Partial | H table | Multiple migration states remain partial or missing. |
| Data / API contract | Partial | DATA-01, MAP-04, MAP-06, REAL-01 | Direction is stated, but the replacement MCP schema is not precise enough yet. |
| Persistence / caching | Partial | DATA-02, REAL-03 | Trust and MCP truth persistence/readers need explicit compatibility rules. |
| Permissions / auth expiry | Missing | DATA-03, H | No explicit credential/expiry handling is defined for the ACP-default trust model. |
| Feature flags / rollout / rollback | Partial | FLAG-01, DATA-04 | Compatibility fallback exists, but cutover/rollback policy remains high-level. |
| Analytics / instrumentation | Partial | METRIC-01 | Gate ownership exists in concept, but not yet in operational detail. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Verification intent exists, but named suites and compatibility proof remain incomplete. |
| Dependencies / integration points | Partial | DOC-02, REAL-04, REAL-05 | Dependency gating is now explicit, but implementation cannot start until the prerequisite closes. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended as a delta over current stable refs and current code, not a greenfield transport rewrite.
- ASSUMP-02: `P030` will either reach Green implementation readiness or be promoted into a stable successor reference before `P033` starts.
- QUESTION-01: what exact fields live inside `backend_profile.mcp_intent`?
- QUESTION-02: how do legacy `server_unverified` / `server_verified` values map forward for historical runs, previews, reports, and comparison?
- QUESTION-03: which Goose compatibility surfaces remain operator-visible after the migration?
- BLOCKER-01: replacement MCP schema is not specific enough to implement safely.
- BLOCKER-02: full Goose owner/surface inventory is not yet locked.
- BLOCKER-03: legacy trust-value migration/back-compat is not yet defined.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
