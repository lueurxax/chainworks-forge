# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md` | 2026-04-08 | High | `P029` now explicitly anchors fail-closed factory changes, transport-neutral MCP registry migration, existing-capability ownership, and disabled-provider rollout semantics on `ConfiguredProvider.isEnabled`. | Review could judge against stale draft text. | Core proposal |
| DOC-02 | `docs/reference/acp-runtime-transport.md` | 2026-04-08 | High | Current stable transport baseline is ACP-shaped core plus Goose compatibility, with first-wave ACP adapters already proven. | Review could judge expansion against the wrong transport baseline. | Core dependency |
| DOC-03 | `docs/reference/provider-platform.md` | 2026-04-08 | High | Provider platform is machine-local, family-first, transport-adjacent, and currently supports `codex`, `claude_code`, `gemini`. | Proposal could reopen provider ownership or transport leakage into operator settings. | Core dependency |
| DOC-04 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-08 | High | MCP truth must preserve requested / predicted / actual / denied layers and stay visible through current shell-owned report/comparison surfaces. | Proposal could under-specify MCP ownership. | Core dependency |
| DOC-05 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-08 | High | Recovery/report readers must prefer persisted execution truth over heuristic reconstruction. | Proposal could weaken execution truth while adding new runtimes. | Adjacent dependency |
| DOC-06 | `docs/reference/live-provider-execution-slice.md` | 2026-04-08 | Medium | Current live slice is transport-selected but intentionally narrow and still bounded around current implemented adapter families. | Proposal could assume broader live-slice guarantees than current repo owns. | Adjacent dependency |
| DOC-07 | `docs/proposals/030-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-08 | Medium | `P030` assumes `P029` proves second-wave providers first, before Goose simplification. | `P029` readiness affects follow-on transport simplification. | Follow-on context |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo shape, execution model, provider/runtime baseline, operator shell | 2026-04-08 | High | Still valid as accelerator, but affected runtime/provider slices required direct code refresh. | Review setup |
| BASE-02 | Proposal-specific integration context | Missing | none | 2026-04-08 | High | No existing `P029.review/integration-context.md` was present. | None blocking |
| BASE-03 | Prior `P029` evidence/review artifacts | Partially refreshed | prior local review basis | 2026-04-08 | High | Prior red and amber bases were refreshed after proposal text changed materially. | Continuity |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - second-wave ACP runtime onboarding for Codex ACP, Auggie CLI ACP, and Junie CLI ACP
  - provider-platform expansion for new families/adapters/settings/readiness
  - fail-closed transport factory behavior
  - transport-neutral MCP registry ownership
  - capability enforcement through existing provider capability truth
  - disabled-provider rollout semantics
  - focused `proposal-029` gate expectation
- Out of scope:
  - Goose removal as canonical transport
  - hard runtime cutover
  - operator-grade claims for second-wave providers
  - MCP parity claims across providers
- Deferred intentionally:
  - follow-on Goose simplification in `P030`
- Assumptions:
  - no hidden feature-flag framework exists outside the checked-in repo
  - current review judges readiness against current repo owners, not against future desired shapes
- Open questions:
  - none blocking
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` add/edit provider flow | Targeted refresh | 2026-04-08 | High | Provider family is a first-class picker and transport is a separate operator choice. Proposal now explicitly keeps disabled second-wave providers visible in Settings and enabled via the same surface. | Implementation could still regress clarity if it leaks transport into family UX. | Provider-platform UI |
| NAV-02 | Provider diagnostics / troubleshooting surfaces | Baseline + targeted refresh | 2026-04-08 | Medium | Current provider readiness UX is family-driven and machine-local. Proposal now explicitly routes disabled-provider truth into readiness/preflight/diagnostics rather than capability mismatch lanes. | Disabled and misconfigured states could still be conflated if implementation drifts. | Readiness UX |
| NAV-03 | `RunReportView` / `RunComparisonView` / recovery readers | Baseline | 2026-04-08 | High | Runtime and MCP truth already extend current shell-owned report/comparison/recovery spine. Proposal preserves these readers. | Proposal must preserve these readers rather than inventing a parallel lane. | Execution truth |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ConfiguredProvider.swift` / `ProviderFamily`, `ProviderDefaults`, `ProviderCapabilities` | Provider platform | Current provider family, model compatibility, capability, and display-name owner | 2026-04-08 | High | Current provider family model is family-first and transport-adjacent; capability truth already exists here. | Proposal must keep this owner model coherent. | Core dependency |
| MAP-02 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Provider platform | Seeds machine-local providers and preferred family bindings | 2026-04-08 | High | Current availability/defaulting is machine-local persisted settings, not repo/catalog flags. Proposal now explicitly ties `isEnabled` to seeding and repair. | Implementation must preserve that single owner. | Rollout owner |
| MAP-03 | `Chainworks Forge/Views/ProviderSettingsView.swift` | UI | Family picker and transport picker surface current operator model | 2026-04-08 | High | Family and transport are separate axes in the current UI. Proposal preserves that separation while adding second-wave providers. | UI drift remains an implementation risk, not a proposal-text gap. | UI fit |
| MAP-04 | `Chainworks Forge/Providers/BackendProfileResolverV2.swift` | Resolution | Resolves backend profile -> configured provider -> runtime profile binding | 2026-04-08 | High | Current resolver already owns `effectiveRuntimeNamespace`. Proposal now also binds disabled-provider failure to this owner path. | Resolver behavior could drift in implementation if not tested. | Core dependency |
| MAP-05 | `Chainworks Forge/Engine/RuntimeTransport.swift` | Transport contract | Defines current factory contract and runtime error enum | 2026-04-08 | High | Proposal explicitly targets this owner for `throws` and `unknownAdapterFamily`. | Closed proposal-text blocker. | Closed blocker |
| MAP-06 | `Chainworks Forge/Engine/ExecutionService.swift` / `DefaultRuntimeTransportFactory` | Execution | Current adapter selection and Goose fallback owner | 2026-04-08 | High | Proposal explicitly targets the real factory owner chain. | Closed proposal-text blocker. | Closed blocker |
| MAP-07 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift` | MCP runtime policy | Current namespace resolution and registry validation owner | 2026-04-08 | High | Proposal targets the real remaining Goose-owned registry seam, not just namespace strings. | Closed proposal-text blocker. | Closed blocker |
| MAP-08 | `Chainworks Forge/Engine/GooseSessionBridge.swift` (`RuntimeSessionBridge`) | Runtime bridge | Current runtime bridge still defaults to `GooseExtensionRegistryReader` | 2026-04-08 | High | Proposal explicitly addresses this registry owner. | Closed proposal-text blocker. | Closed blocker |
| MAP-09 | `scripts/test-gate.sh` | Verification | Current focused proposal gate registry | 2026-04-08 | High | No `proposal-029` gate exists yet, and the proposal explicitly requires one. | Implementation still needs proof coverage, but the proposal text is clear enough. | Verification target |
| MAP-10 | `Chainworks Forge/Providers/ProviderRegistry.swift` / `preferredProvider(for:)` | Provider platform | Current family-level provider selection owner | 2026-04-08 | High | Current selection ignores any disabled-instance concept and returns preferred-or-first by family. Proposal now explicitly binds `isEnabled` to filtering, repair, and nil-on-no-enabled semantics. | Closed proposal-text blocker. | Closed blocker |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Machine-local provider settings | `ProviderSettingsStore.swift`, `provider-platform.md` | Persisted config | 2026-04-08 | High | Provider enablement/defaulting lives in machine-local persisted settings. Proposal now keeps rollout semantics there via `ConfiguredProvider.isEnabled`. | Implementation must not invent a second rollout flag lane. | Core rollout owner |
| DATA-02 | MCP install/readiness registry | `RuntimeExtensionRegistryProvider`, `MCPPolicyRuntime.swift`, `GooseSessionBridge.swift` | Runtime validation | 2026-04-08 | High | Proposal explicitly targets the real Goose-owned registry abstraction. | Closed proposal-text blocker. | Closed blocker |
| DATA-03 | Runtime/provider settlement truth | `BackendProfileResolverV2.swift`, `AgentExecution`, `RunReportBuilder`, `RunComparisonService` | Persisted execution truth | 2026-04-08 | High | Runtime profile, adapter family, and capability class already persist into reports/comparison. Proposal continues to extend existing truth lanes. | Truth continuity must remain intact in implementation. | Truth continuity |
| DATA-04 | Preferred provider family binding | `ProviderRegistry.swift`, `ProviderSettingsStore.swift` | Selection + repair | 2026-04-08 | High | Proposal now explicitly names filtering, repair, seeded defaults, resolver failure, preflight treatment, and diagnostics/report behavior for disabled providers. | Closed proposal-text blocker. | Closed blocker |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Transport factory owner path | Current repo | 2026-04-08 | High | Proposal targets the real non-throwing factory contract and error path. | No longer a live proposal-text blocker. | Closed blocker |
| INT-02 | MCP validation owner path | Current repo | 2026-04-08 | High | Proposal targets the real remaining Goose-owned registry seam. | No longer a live proposal-text blocker. | Closed blocker |
| INT-03 | Provider family / transport operator model | Baseline + current repo | 2026-04-08 | High | Proposal consistently uses `.codexACP`, `.auggie`, `.junie` and keeps transport/provider distinction explicit. | Old identity drift is closed. | Closed blocker |
| INT-04 | Capability truth owner | Baseline + current repo | 2026-04-08 | High | Proposal extends existing `ProviderCapabilities` instead of inventing a second capability authority. | No longer a live proposal-text blocker. | Closed blocker |
| INT-05 | Verification gate owner | Current repo | 2026-04-08 | High | Current gate registry still has no `proposal-029` lane, but the proposal now clearly requires one. | Implementation follow-through remains required. | Operational follow-up |
| INT-06 | Enablement owner vs family selection | Current repo | 2026-04-08 | High | Proposal now explicitly binds `ConfiguredProvider.isEnabled` to the current family-level selection, repair, resolver, preflight, and settings/diagnostics surfaces. | No live proposal-text blocker remains. | Closed blocker |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, MAP-04, MAP-06 | catalog -> resolver -> factory | Core intended path is explicit. |
| Happy path | Specified | DOC-01, MAP-04, MAP-06, MAP-07 | provider onboarding + runtime selection + MCP resolution | Core intended path is explicit. |
| Loading | Specified | NAV-01, NAV-02 | settings / diagnostics refresh surfaces | Disabled and enabled provider states are explicitly routed. |
| Empty | Specified | NAV-01, NAV-02 | settings / readiness surfaces | Disabled-by-default second-wave providers are explicitly visible but not active. |
| Validation error | Specified | DOC-01, MAP-05, MAP-07 | preflight, factory, MCP resolution | Factory/MCP/capability failures are structurally owned. |
| Backend error | Specified | DOC-01, MAP-06, MAP-07 | factory, MCP registry, provider health | Rollout-disabled and capability failures are explicitly separated. |
| Offline / degraded | Partial | DOC-03, NAV-02 | provider diagnostics / health | High-level handling is present; fine-grained implementation UX remains future work. |
| Retry / recovery | Specified | DOC-05, NAV-03 | reports / comparison / recovery readers | Truth continuity is preserved. |
| Auth / permission expiry | Partial | DOC-03, NAV-02 | provider auth/setup surfaces | New-provider auth/setup lifecycle remains intentionally high-level. |
| Rollback / cancellation | Specified | DOC-01, DATA-01, DATA-04 | provider settings / enablement | Disable path and repair semantics are explicitly owned. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | `ConfiguredProvider.isEnabled` | machine-local provider enablement | second-wave providers seed disabled and become active only through Settings | disable provider instance and clear/repair effective selection | 2026-04-08 | High | Single-owner direction is now explicit and implementation-ready. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Proposal-focused verification gate | onboarding proof | focused `proposal-029` gate | 2026-04-08 | Medium | The proposal names the required proof lane. Extra rollout analytics are optional follow-on work, not a readiness blocker. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Focused proposal gate | second-wave runtime onboarding | No current `proposal-029` gate exists | `proposal-029` gate in `test-gate.sh` | 2026-04-08 | High | The proof requirement is now explicit enough for implementation. |
| TEST-02 | Unit/integration | factory, MCP, provider platform | Current repo has `Proposal026Tests`, `Proposal025Tests`, `ProviderPlatformTests`, `GooseServerTransportTests` | Extend existing suites plus new `Proposal029Tests` | 2026-04-08 | Medium | Suite composition is an implementation detail, not a remaining proposal-text blocker. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Enablement owner | `ConfiguredProvider.isEnabled` is the single rollout owner and is explicitly tied to family-level filtering, repair, resolver failure, preflight, and diagnostics | current repo still lacks the implementation, but the proposal now names the exact owner chain | 2026-04-08 | High | Prior proposal-text blocker is closed. |
| REAL-02 | Transport factory | proposal changes the protocol/error/preflight owner chain | current repo is still non-throwing and Goose-fallback | 2026-04-08 | High | Closed at proposal-text level; now an implementation task. |
| REAL-03 | MCP registry | proposal targets the Goose-owned registry seam directly | current repo still uses Goose-specific registry types | 2026-04-08 | High | Closed at proposal-text level; now an implementation task. |
| REAL-04 | Capability enforcement | proposal reuses `ProviderCapabilities` as the authority | current repo capability truth already lives there | 2026-04-08 | High | Closed at proposal-text level. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | The motivating gaps are clear. |
| Scope boundaries | Specified | DOC-01 | Internal scope wording drift is closed. |
| Reusable baseline coverage | Specified | BASE-01, DOC-02, DOC-03, DOC-04 | Baseline coverage is strong enough with targeted code refresh. |
| Screen / surface definition | Specified | NAV-01, NAV-02 | Settings/readiness/diagnostics rollout behavior is explicitly owned. |
| Navigation / entry points | Specified | NAV-01, NAV-02 | Entry surfaces are clear enough for implementation. |
| State handling | Specified | H table | Proposal now explicitly covers disabled-provider states. |
| Data / API contract | Specified | DATA-01, DATA-02, DATA-03, DATA-04 | Main owner seams are explicit. |
| Persistence / caching | Specified | DATA-01, DATA-04 | `isEnabled` persistence owner and repair semantics are explicit. |
| Permissions / auth expiry | Partial | DOC-03, NAV-02 | New-provider auth/setup lifecycle remains intentionally high-level. |
| Feature flags / rollout / rollback | Specified | FLAG-01, REAL-01 | Single-owner rollout model is explicit. |
| Analytics / instrumentation | Partial | METRIC-01 | Focused proof gate is explicit; extra instrumentation is optional. |
| Testing strategy | Specified | TEST-01, TEST-02 | Testing direction is explicit enough for implementation. |
| Dependencies / integration points | Specified | INT-01, INT-02, INT-03, INT-04, INT-06 | Prior core dependency-chain blockers are closed. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The review assumes current checked-in provider-platform and ACP runtime seams are the authoritative implementation baseline.
- ASSUMP-02: The review assumes no hidden rollout-flag system exists outside checked-in repo code/config.
- BLOCKER-01: None.

## O. Research Triggers / External Questions
No external research triggers were needed for this local readiness pass.
