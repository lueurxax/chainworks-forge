# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-09 | High | `P033` now includes a hard `P030` prerequisite gate, richer MCP migration text, broader Goose inventory, operator-surface migration, and legacy trust fallback. | The review could keep stale blockers that the proposal has already fixed. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-09 | High | `P030` is still `Not Implemented` / `Not Ready`. | `P033` could be judged as immediately executable when its prerequisite is still externally blocked. | Dependency status. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | Stable review intake should start from current reference docs, then do narrow code refresh. | Review could redo unnecessary archaeology or miss reusable subsystem truth. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current baseline still includes live Goose-backed execution, ACP-shaped runtime transport, repo-owned MCP truth, and Goose remediation/operator surfaces. | Proposal could be reviewed as if those current owners were already retired. | Current host-system truth. |
| DOC-05 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Current transport seam is ACP-shaped, but live runtime still carries Goose compatibility paths. | Proposal could overstate how much simplification is already complete. | Runtime baseline. |
| DOC-06 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-09 | High | Current MCP truth includes required/optional extensions, fallback semantics, runtime mappings, and frozen requested/predicted/actual/denied truth. | Proposal could still flatten the MCP contract too much. | MCP ownership baseline. |
| DOC-07 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Provider settings/readiness and Goose remediation are stable product surfaces. | Proposal could miss operational compatibility surfaces. | Settings/readiness baseline. |
| DOC-08 | `docs/reference/workflow-execution-engine.md` | 2026-04-09 | High | Current compile/runtime model still freezes `mcp_profile` on agents and documents Goose-backed live execution. | Proposal could understate how deep current MCP assumptions run. | Engine baseline. |
| DOC-09 | `docs/reference/operator-experience.md` | 2026-04-09 | High | Operator runtime provenance is still rooted in legacy Goose-era trust values. | Proposal could rename trust without accounting for reader compatibility. | Operator trust baseline. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level baseline posture and stable-reference map | 2026-04-09 | High | Still fresh for review setup. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current implemented product boundaries and stable subsystem map | 2026-04-09 | High | Fresh overall; the affected transport/MCP slice still needed code refresh. | Current system map. |
| BASE-03 | stable runtime/provider/MCP reference docs | Partially refreshed | transport, provider, MCP, operator shell | 2026-04-09 | High | Stable refs remain valid, but proposal-critical dual-path MCP behavior required direct code inspection. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-09 | High | No `033...review/integration-context.md` exists. Not blocking because stable refs plus targeted code refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Reused | prerequisite runtime/provider proof status | 2026-04-09 | High | Fresh enough to establish dependency status. | Dependency check. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - remove Goose as canonical runtime transport path
  - ACP-first dispatch simplification
  - Goose compatibility-only packaging
  - phased MCP ownership migration
  - operator-surface migration
  - legacy trust normalization
  - proposal-specific proof gating
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from all system-level settings
  - weakening execution/recovery/report truth
- Deferred intentionally:
  - full Goose removal from operator workflows
- Assumptions:
  - `P033` is a delta over current stable refs and current HEAD
  - the user asked for proposal readiness, not runtime verification or product review
- Open questions:
  - does Phase 1 dual-path preserve legacy MCP for ACP-backed agents as well as Goose-backed ones?
  - what is the final canonical owner of `mcp_server_registry` after `P033` completes?
  - what exact suite composition will make `proposal-033` operationally meaningful?
- Proposal-first blockers:
  - Phase 1 MCP dual-path wording is narrower than current runtime reality
  - final `mcp_server_registry` authority is still ambiguous
  - `proposal-033` verification lane is still not concretely composed
- External hold:
  - `P030` remains red, so implementation cannot start yet even if `P033` proposal quality improves

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Targeted refresh | 2026-04-09 | High | Settings still includes Goose-first setup and compatibility affordances. | Proposal must keep compatibility UX coherent during migration. | Operator setup surface. |
| NAV-02 | `FirstRunSetupWizard` | Targeted refresh | 2026-04-09 | High | First-run still offers Goose-first setup paths for provider families. | Proposal must preserve setup truth during the transition. | Primary onboarding journey. |
| NAV-03 | `PilotReadinessView` | Targeted refresh | 2026-04-09 | High | Readiness still exposes Goose compatibility and per-provider troubleshooting. | Proposal must keep readiness operator-grade while demoting Goose canonically. | Readiness journey. |
| NAV-04 | `RunsHomeView` runtime provenance badge | Targeted refresh | 2026-04-09 | High | Main shell still renders legacy `server_unverified` / `server_verified` trust labels. | Proposal must preserve historical legibility under new trust vocabulary. | Operator trust/history surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime session bridge | MCP resolution before transport-specific request shaping | 2026-04-09 | High | `RuntimeSessionBridge` resolves MCP policy before ACP/Goose branching and then derives ACP `mcpServers` from the same report. | Proposal can mis-specify dual-path MCP migration if it treats the old path as Goose-only. | Main Phase 1 architecture seam. |
| MAP-02 | `Chainworks Forge/Engine/ACPAdapters/*.swift` | ACP transports | native ACP `mcpServers` consumption | 2026-04-09 | High | Current ACP transports already accept server injection via `mcpServers`. | Proposal must preserve legacy MCP input for ACP too until migration is proven. | ACP dual-path proof seam. |
| MAP-03 | `Chainworks Forge/DSL/AgentCatalog.swift` and `Chainworks Forge/Engine/MCPPolicyRuntime.swift` | DSL / policy | canonical MCP contract and runtime mapping | 2026-04-09 | High | Current contract still includes richer repo-owned MCP truth than `required servers` alone. | Proposal can still under-specify the target schema or end-state owner. | Main data-model boundary. |
| MAP-04 | `Chainworks Forge/Models/Run.swift`, `Chainworks Forge/Engine/RunReportBuilder.swift`, `Chainworks Forge/Views/RunsHomeView.swift` | Persistence / report / shell | legacy trust and frozen MCP truth readers | 2026-04-09 | High | Historical trust values and frozen MCP truth are persisted and then rendered later. | Proposal must preserve reader behavior and proof it explicitly. | Reader-compatibility boundary. |
| MAP-05 | `scripts/test-gate.sh` and `docs/reference/test-gates.md` | Verification | proposal-specific proof-lane ownership | 2026-04-09 | High | No concrete `proposal-033` gate exists yet. | Proposal verification can remain too loose without named suites/evidence outputs. | Verification boundary. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Legacy MCP declaration path | `AgentCatalog.swift`, `MCPPolicyRuntime.swift`, `GooseSessionBridge.swift` | Repo-owned schema feeding all runtimes | 2026-04-09 | High | Current `agent.mcp_profile` path still feeds both Goose and ACP runtime execution. | Proposal could mis-specify dual-path survival rules. | Main architecture blocker. |
| DATA-02 | Registry ownership | `AgentCatalog.swift`, `per-agent-mcp-policy-and-runtime-validation.md` | Repo-owned runtime mapping | 2026-04-09 | High | `mcp_server_registry` is still current canonical runtime-namespace mapping truth. | Proposal Phase 3 cannot be judged safely without a locked final owner. | End-state blocker. |
| DATA-03 | Persisted trust / MCP truth | `Run.swift`, `RunReportBuilder.swift`, `RunsHomeView.swift` | Persisted truth and read-side mapping | 2026-04-09 | High | Reader fallback is now proposal-owned and must be proven. | Proposal gate needs explicit coverage for reader compatibility. | Proof boundary. |
| DATA-04 | Dependency proof lane | `030...IMPLEMENTATION_AUDIT_R4.md` | External proposal dependency | 2026-04-09 | High | `P030` is still red. | `P033` remains operationally blocked even if proposal quality rises. | External hold. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Dual-path runtime MCP resolution | Current repo | 2026-04-09 | High | Legacy MCP truth is resolved once and then consumed by both Goose and ACP request shaping. | Proposal Phase 1 still reads too Goose-specific. | Architecture blocker. |
| INT-02 | Registry ownership | Baseline + current repo | 2026-04-09 | High | Repo-owned `mcp_server_registry` is still the canonical mapping authority today. | Proposal end state remains ambiguous. | Canonical contract blocker. |
| INT-03 | Trust readers | Baseline + current repo | 2026-04-09 | High | Reader-side compatibility is needed because historical runs and preview surfaces still use legacy trust strings. | Proposal now addresses this, but the proof lane must cover it. | UX/report proof seam. |
| INT-04 | Proposal-specific proof gate | Current repo + proposal | 2026-04-09 | High | The proposal owns a `proposal-033` gate, but the gate is not yet operationally defined. | Verification remains partial until suites/evidence outputs are named. | Readiness blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02, NAV-03 | Settings, first-run wizard, readiness | Major entry surfaces are now named. |
| Happy path | Partial | DOC-01, MAP-01, MAP-02, INT-01 | runtime dispatch, dual-path MCP | Happy path is clearer, but legacy MCP survival for ACP is still under-specified. |
| Loading | Partial | DOC-01, NAV-03 | readiness refresh, troubleshooting | Operator direction is present, but loading-proof expectations are not central to the remaining blockers. |
| Empty | Partial | DOC-01, NAV-01 | provider setup surfaces | Compatibility-only empty states remain high-level. |
| Validation error | Partial | DOC-01, MAP-03, DATA-01 | YAML validation, preflight | The new MCP path exists, but precedence and cross-runtime survival rules are still partial. |
| Backend error | Partial | DOC-01, NAV-03, DATA-03 | transport/runtime failures, trust readers | Proposal addresses recovery vocabulary, but proof ownership remains partial. |
| Offline / degraded | Partial | DOC-01, NAV-03 | readiness, compatibility | Major direction exists. |
| Retry / recovery | Deferred intentionally | DOC-01, DOC-04 | execution truth/recovery refs | No new recovery model is introduced. Acceptable. |
| Auth / permission expiry | Deferred intentionally | DOC-01, DOC-07 | provider troubleshooting | Not central to the remaining proposal blockers. |
| Rollback / cancellation | Partial | DOC-01, DATA-04 | compatibility fallback | Rollback remains high-level and tied to external dependency hold. |
| Historical-read compatibility | Specified | DOC-01, DOC-09, MAP-04, INT-03 | reports, RunsHome, previews | Reader fallback is now explicitly described. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Current provider `isEnabled` rollout gates and Goose compatibility presence | provider/runtime selection | proposal shifts defaults away from Goose while preserving compatibility fallback | rollback path is still mostly carried by compatibility retention and `P030` hold | 2026-04-09 | Medium | Operationally acceptable for now; not the main remaining blocker. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | readiness proof | rollout/proof sequence in `P033` | 2026-04-09 | High | Gate intent exists, but concrete suite composition and evidence outputs are still missing. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite implementation audit | second-wave ACP readiness | `P030` audit proves the prerequisite is still not fully implemented | `P033` correctly hard-gates on `P030` Green readiness | 2026-04-09 | High | External hold remains, but it is no longer a proposal contradiction. |
| TEST-02 | focused proposal gate | post-Goose canonical transport | no `proposal-033` gate exists yet | add `proposal-033` with named suites for dual-path MCP, trust reader fallback, and Goose compatibility proof | 2026-04-09 | High | Verification plan is still too conceptual. |
| TEST-03 | dual-path MCP proof | Phase 1 migration behavior | current code still relies on legacy MCP for ACP and Goose | require explicit proof that ACP + Goose legacy paths both survive until migration is complete | 2026-04-09 | High | Main proposal gap. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Phase 1 MCP dual-path | old MCP path continues unchanged for Goose-backed runs | current runtime resolves MCP once and uses it for ACP `mcpServers` too | 2026-04-09 | High | Proposal wording is still too narrow for current runtime reality. |
| REAL-02 | Phase 3 registry end state | `mcp_server_registry` may stay or may move machine-local depending on adapters | current baseline treats repo-owned registry as canonical truth | 2026-04-09 | High | Proposal still leaves final architecture ambiguous. |
| REAL-03 | Focused proof ownership | `proposal-033` gate will validate the slice | no concrete gate composition exists yet | 2026-04-09 | High | Verification remains partial. |
| REAL-04 | External dependency | `P033` implementation starts only after `P030` is green | `P030` is still red today | 2026-04-09 | High | Operational start remains blocked by an explicit prerequisite hold. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | In-scope and out-of-scope are explicit. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05, DOC-06, DOC-07, DOC-08, DOC-09 | Stable local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03, NAV-04 | Major operator surfaces are now named. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Dual-path and verification states remain partial. |
| Data / API contract | Partial | DATA-01, DATA-02, REAL-01, REAL-02 | Direction is solid, but dual-path scope and final registry authority remain unresolved. |
| Persistence / caching | Specified | DATA-03 | Legacy trust reader compatibility is now explicit. |
| Permissions / auth expiry | Deferred intentionally | DOC-01, DOC-07 | Not central to this proposal’s remaining blockers. |
| Feature flags / rollout / rollback | Partial | FLAG-01, DATA-04 | External dependency hold is explicit, but exact cutover proof remains partial. |
| Analytics / instrumentation | Partial | METRIC-01 | Gate intent exists, but operational composition is still missing. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Focused proof lane still needs exact suite/evidence definition. |
| Dependencies / integration points | Partial | DOC-02, REAL-04 | Dependency handling is explicit, but the prerequisite is still externally unresolved. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended as a delta over current stable refs and current code, not a greenfield transport rewrite.
- ASSUMP-02: `P030` will be rereviewed independently before `P033` implementation starts.
- QUESTION-01: when both `agent.mcp_profile` and `backend_profile.mcp_intent` are present, which source is canonical in Phase 1?
- QUESTION-02: does `P033` end with repo-owned `mcp_server_registry`, or is registry relocation intentionally deferred?
- QUESTION-03: what exact test suites and evidence outputs make up `proposal-033`?
- BLOCKER-01: Phase 1 dual-path wording is still too narrow for current ACP runtime behavior.
- BLOCKER-02: Phase 3 final authority for runtime-namespace MCP mapping is still ambiguous.
- BLOCKER-03: proposal-owned proof lane is still not operationally composed.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot start yet.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
