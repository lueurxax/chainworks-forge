# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie.md` | 2026-04-09 | High | `P029` is now an implementation-aligned delta proposal over current HEAD, not a greenfield runtime-profile draft. It explicitly records already-landed catalog/provider work and the remaining transport, MCP, and proof tasks. | Review could judge stale intent instead of the current proposal contract. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | The reusable baseline says proposal review should start from stable reference docs and only refresh the affected runtime/provider slice. | Review could redo full-system archaeology instead of a narrow delta review. | Baseline intake. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current HEAD already has ACP transport, per-agent MCP truth, provider platform, operator shell, and design-system baselines. `P029` must therefore behave as a delta over implemented system truth. | Proposal could be judged as if those subsystem owners were still undefined. | Current host-system truth. |
| DOC-04 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Provider/platform is already a product surface with `ProviderSettingsView`, `PilotReadinessView`, diagnostics, troubleshooting, and machine-local configuration owners. | Proposal could under-specify operator-facing rollout and remediation behavior. | Provider/UI baseline. |
| DOC-05 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Stable ACP transport baseline still describes first-wave adapters only and treats future second-wave expansion as separate work. | Proposal could blur baseline truth with future-state intent. | Transport baseline. |
| DOC-06 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-09 | High | MCP truth is catalog-owned and operator-visible, but current runtime realization still distinguishes Goose-backed registry validation from ACP-native realization. | Proposal could overclaim transport-neutral MCP behavior without a concrete owner plan. | MCP baseline. |
| DOC-07 | `docs/proposals/029-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R2.md` | 2026-04-09 | High | Same-tree implementation audit says second-wave provider platform/capabilities mostly landed, while transports are still stubs and registry ownership remains only partially generalized. | Proposal review could miss current repo contradictions that the proposal now needs to absorb cleanly. | Current repo reality check. |
| DOC-08 | `docs/reviews/029-acp-second-wave-runtime-profiles-codex-auggie-junie-review.md` and `docs/reviews/029-acp-second-wave-runtime-profiles-codex-auggie-junie-evidence-pack.md` | 2026-04-09 | High | Prior local review artifacts called the proposal Green, but they predate the 2026-04-09 implementation-aligned amendments and are now stale for readiness judgment. | Review could inherit an outdated Green call. | Continuity and staleness check. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level baseline posture and stable reference map | 2026-04-09 | High | Still fresh for review setup; affected runtime/provider slice needed direct refresh. | Review entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current implemented product boundaries and canonical subsystem docs | 2026-04-09 | High | Fresh for affected surfaces. | Current system map. |
| BASE-03 | Proposal-specific integration context | Missing | none | 2026-04-09 | High | No `029...review/integration-context.md` exists. Not blocking because subsystem refs plus targeted code refresh were enough. | None blocking. |
| BASE-04 | Prior `029` review artifacts | Partially refreshed | earlier proposal-readiness intake | 2026-04-09 | High | Stale after current proposal amendments; reused only as continuity, not as readiness truth. | Historical trail. |
| BASE-05 | `029` implementation audit `R2` | Partially refreshed | current code-path contradictions for provider/runtime/MCP slice | 2026-04-09 | High | Fresh enough for repo-reality deltas; not used as a substitute for proposal review. | Current repo alignment. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - second-wave ACP runtime onboarding for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`
  - provider-platform expansion for new families, seeded settings, adapters, and rollout gating
  - fail-closed transport selection
  - MCP registry ownership and runtime-namespace alignment
  - capability enforcement through `ProviderCapabilities`
  - focused proof gate for `proposal-029`
- Out of scope:
  - Goose removal as canonical transport
  - hard cutover away from Goose
  - operator-grade claims for second-wave providers
  - generic cross-provider MCP parity
- Deferred intentionally:
  - transport simplification in `P030`
- Assumptions:
  - `P029` is judged as a delta over current stable refs, not as a replacement for them
  - the user requested proposal readiness only, not a product overlay
- Open questions:
  - whether Auggie/Junie are meant to ship with actual MCP lanes or intentionally remain zero-MCP-only in this proposal
  - whether `P029` is one proposal for all three live transports or a structural slice plus later execution phases
- Blockers:
  - none at evidence-gate level

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Targeted refresh | 2026-04-09 | High | Provider family is already a first-class operator axis and current provider entries surface troubleshooting and preference controls. `P029` adds more families to this existing shell-owned surface. | Proposal could under-specify how staged second-wave providers appear in Settings. | UI fit for enablement. |
| NAV-02 | `PilotReadinessView` and `ProviderTroubleshootingPanel` | Baseline + targeted refresh | 2026-04-09 | High | Provider platform already owns readiness and troubleshooting UX. `P029` touches rollout and MCP readiness, so these surfaces need a shared state taxonomy, not only Settings toggles. | Proposal could leave rollout/remediation semantics fragmented. | UX/readiness fit. |
| NAV-03 | Run-start preflight | Baseline + targeted refresh | 2026-04-09 | High | `PreflightService` already surfaces provider binding, capability checks, and runtime registry status. `P029` makes preflight a primary operator-facing gate for second-wave runtimes. | Proposal could stop at internal owner changes without a coherent operator-facing outcome model. | Core run-start journey. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider platform | owns provider families, capability truth, and `isEnabled` rollout state | 2026-04-09 | High | Current repo already has `.codexACP`, `.auggie`, `.junie`, `supportsMCPReconciliation`, and `isEnabled`. | Proposal must behave as delta over existing owners. | Core provider truth. |
| MAP-02 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Provider platform | seeds defaults and repairs preferred providers | 2026-04-09 | High | Second-wave providers already seed disabled by default. Removal repair now filters by `isEnabled`. | Proposal cannot describe this area as wholly future-state. | Rollout semantics. |
| MAP-03 | `Chainworks Forge/Providers/ProviderRegistry.swift` and `BackendProfileResolverV2.swift` | Resolution | family-level selection, disabled-provider failure path, runtime namespace derivation | 2026-04-09 | High | Disabled-provider resolution and second-wave runtime namespaces already exist. | Proposal must focus on remaining gaps, not re-open solved owners. | Binding truth. |
| MAP-04 | `Chainworks Forge/Engine/RuntimeTransport.swift` and `ExecutionService.swift` | Transport | fail-closed factory seam and adapter selection | 2026-04-09 | High | Unknown adapter families already throw, and second-wave family cases are registered in the factory. | Proposal-text must not keep talking like this is still only planned. | Transport delta. |
| MAP-05 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift`, `GooseSessionBridge.swift`, and `PreflightService.swift` | MCP / runtime validation | runtime registry provider selection and operator-facing validation | 2026-04-09 | High | Type rename landed, but live registry loading still resolves through `GooseExtensionRegistryReader`, and preflight still loads one runtime registry path. | Proposal must lock the missing registry owner contract precisely. | Main architecture risk. |
| MAP-06 | `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift`, `AuggieCLIACPTransport.swift`, `JunieCLIACPTransport.swift` | ACP transports | end-to-end second-wave execution | 2026-04-09 | High | All three transports still throw `"is not yet implemented"` on create/stream/close. | Proposal completeness depends on clear scope and proof rules for these adapters. | Main implementation gap. |
| MAP-07 | `examples/agents/agents.yaml` | Catalog/runtime intent | canonical runtime profiles, backend profiles, and MCP server registry | 2026-04-09 | High | `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp` runtime profiles already exist, but rich MCP runtime mappings are currently present for `codex` only. | Proposal must say whether Auggie/Junie ship with MCP lanes or intentionally without them. | Catalog contract. |
| MAP-08 | `Chainworks ForgeTests/Proposal029Tests.swift` and `scripts/test-gate.sh` | Verification | focused proof lane and proposal-owned regression slice | 2026-04-09 | High | `proposal-029` gate exists and the focused test suite exists. | Proposal proof contract must align with what the gate is supposed to prove. | Verification contract. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Machine-local rollout state | `ProviderSettingsStore.swift`, `provider-platform.md` | Persisted config | 2026-04-09 | High | `ConfiguredProvider.isEnabled` is already persisted locally and used in selection. | Proposal should not invent a second rollout owner. | Rollout boundary. |
| DATA-02 | Runtime registry snapshot | `MCPPolicyRuntime.swift`, `PreflightService.swift`, `GooseSessionBridge.swift` | Runtime validation | 2026-04-09 | High | Registry abstraction name is generalized, but the concrete source remains Goose-centric. | Proposal must specify concrete owner/source rules for second-wave runtimes. | Registry boundary. |
| DATA-03 | Frozen runtime/provider truth | `provider-binding-truth.md`, `execution-truth-and-recovery.md`, `RunReportBuilder` via stable refs | Persisted run truth | 2026-04-09 | High | Existing run/report truth should remain authoritative regardless of new adapter families. | Proposal cannot weaken persisted truth while expanding transports. | Report/recovery continuity. |
| DATA-04 | Provider setup and auth readiness | `provider-platform.md`, provider adapters | Setup/readiness | 2026-04-09 | Medium | Provider platform already owns health and troubleshooting, but `P029` stays high-level on second-wave executable/auth readiness specifics. | Implementation may improvise auth/remediation UX. | Readiness completeness. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider settings + pilot readiness shell | Baseline + current repo | 2026-04-09 | High | Provider onboarding is already an operator workflow, not hidden runtime glue. | Proposal needs explicit cross-surface state and remediation semantics. | UI/UX contract. |
| INT-02 | Runtime transport factory | Current repo | 2026-04-09 | High | Fail-closed registration is already in repo. | Proposal must not mix already-landed prerequisites with future live-transport work ambiguously. | Scope ownership. |
| INT-03 | MCP runtime validation | Current repo | 2026-04-09 | High | Runtime namespace derivation exists, but concrete registry-provider ownership is still unresolved for second-wave families. | Proposal still leaves too much room for ad hoc registry implementation. | Main architecture seam. |
| INT-04 | Canonical catalog + runtime registry | Current repo | 2026-04-09 | High | Catalog already preserves Codex MCP mappings, but Auggie/Junie MCP behavior is still undefined in canonical runtime IDs. | Proposal must choose between explicit MCP mappings and zero-MCP-only rollout for those families. | Main contract gap. |
| INT-05 | Proposal-specific proof gate | Current repo | 2026-04-09 | High | Focused gate exists, but proposal text still leaves uneven proof expectations across the three in-scope transports. | Proposal could be marked complete with weaker proof than its scoped surface implies. | Readiness quality. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-03 | Settings, preflight, runtime profile selection | Entry path is explicit. |
| Happy path | Partial | DOC-01, MAP-06, MAP-08, INT-05 | second-wave execution | Happy-path execution is in scope, but rollout-order wording and proof requirements are inconsistent. |
| Loading | Partial | NAV-01, NAV-02 | settings and readiness refresh | Proposal names settings/preflight/report, but not one shared loading/readiness contract across provider surfaces. |
| Empty | Specified | DOC-01, NAV-01 | disabled second-wave providers visible in Settings | Empty/disabled staged state is intentionally present. |
| Validation error | Specified | DOC-01, MAP-03, MAP-04, MAP-05 | resolver, preflight, factory | Provider-not-enabled and unknown-adapter flows are explicit. |
| Backend error | Partial | DOC-01, MAP-05, MAP-06 | registry failure, transport stub, auth/readiness | Proposal does not yet unify broken transport, missing registry, and provider misconfiguration remediation. |
| Offline / degraded | Partial | DOC-04, NAV-02, DATA-04 | readiness/troubleshooting | Provider health exists in baseline, but second-wave degraded/offline handling remains high-level. |
| Retry / recovery | Deferred intentionally | DOC-03, DATA-03 | existing run truth / recovery docs | `P029` extends runtime truth but does not own a new recovery model. |
| Auth / permission expiry | Partial | DOC-04, DATA-04 | provider setup/readiness | Proposal does not lock second-wave auth/remediation specifics. |
| Rollback / cancellation | Specified | DOC-01, DATA-01 | disable provider / repair preference | Rollback path is disable-and-repair, but cross-surface operator copy remains partial. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | `ConfiguredProvider.isEnabled` | machine-local provider enablement | second-wave providers seed disabled and become eligible through Settings | disable provider and repair preferred binding to next enabled provider or clear it | 2026-04-09 | High | Single rollout owner is already largely present in code and should remain canonical. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Focused `proposal-029` gate | proposal-owned readiness proof | same-tree gate in `scripts/test-gate.sh` | 2026-04-09 | High | Proposal still needs a clearer statement of what this gate proves for each second-wave transport. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Focused proposal gate | provider/runtime/MCP regression slice | `proposal-029` gate exists | keep same-tree focused gate and align it to final proposal scope | 2026-04-09 | High | Proof scope is currently weaker than full proposal scope. |
| TEST-02 | Unit / integration | families, factory, namespace, capability, disabled-provider filtering, catalog mappings | `Proposal029Tests.swift` exists | extend tests to the final chosen MCP and execution completion contract | 2026-04-09 | High | Current tests prove structural pieces, not full end-to-end execution for all three transports. |
| TEST-03 | Execution proof | real second-wave adapter execution | current code still has stub transports | define exact proof requirement for Codex/Auggie/Junie | 2026-04-09 | High | Proposal currently mixes one-provider proof with three-provider scope. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | provider platform and catalog | `P029` is an expansion slice that must preserve already-landed cutovers and capability enforcement | second-wave families, disabled providers, capability enforcement, and catalog entries are already in repo | 2026-04-09 | High | Proposal is correctly a delta over current HEAD, not a greenfield slice. |
| REAL-02 | second-wave execution | scope and AC-11 imply all three transports become executable | `CodexACPTransport`, `AuggieCLIACPTransport`, and `JunieCLIACPTransport` are still stubs | 2026-04-09 | High | Proposal must be precise about what remains in this proposal and what proof is required. |
| REAL-03 | MCP registry ownership | proposal wants transport-neutral registry ownership | concrete registry provider path still resolves through `GooseExtensionRegistryReader` and one runtime-registry load | 2026-04-09 | High | Proposal still needs a clearer second-wave registry authority contract. |
| REAL-04 | rollout order | scope and `3.2` say second-wave transports are in this proposal | `4.7` says only structural prerequisites are “this proposal”, while Codex/Auggie/Junie adapters live in later phases | 2026-04-09 | High | Internal proposal ownership is contradictory. |
| REAL-05 | proof requirements | acceptance says all three routes should stop failing with stub errors | `3.2` only requires one successful Codex proof path plus explicit expectations for Auggie/Junie | 2026-04-09 | High | Verification contract is weaker than proposal scope. |
| REAL-06 | Auggie/Junie MCP behavior | each second-wave provider gets an explicit runtime namespace | canonical `mcp_server_registry` preserves rich runtime mappings for `codex`, but not for `auggie` or `junie` | 2026-04-09 | High | Proposal must decide whether Auggie/Junie ship with MCP lanes or remain zero-MCP-only by design. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-07 | The motivating repo gap is clear. |
| Scope boundaries | Partial | DOC-01, REAL-04 | Internal rollout-order wording conflicts with the stated in-scope surface. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-03, DOC-04, DOC-05, DOC-06 | Stable reference intake is strong enough. |
| Screen / surface definition | Partial | NAV-01, NAV-02, NAV-03, INT-01 | Operator-facing state contract across provider surfaces is still high-level. |
| Navigation / entry points | Partial | NAV-01, NAV-02, NAV-03 | Core entry points are clear, but remediation path ownership is incomplete. |
| State handling | Partial | H table | Non-happy-path readiness/remediation remains only partly locked. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-04, REAL-03, REAL-06 | Registry authority and Auggie/Junie MCP behavior are still under-specified. |
| Persistence / caching | Specified | DATA-01, DATA-03 | Persisted rollout and runtime truth owners are clear enough. |
| Permissions / auth expiry | Partial | DOC-04, DATA-04 | Second-wave setup/auth lifecycle is not locked in detail. |
| Feature flags / rollout / rollback | Specified | FLAG-01, MAP-01, MAP-02, MAP-03 | Single rollout owner is explicit enough. |
| Analytics / instrumentation | Partial | METRIC-01 | Proposal has a proof gate, but not a fully aligned proof matrix. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03, REAL-05 | Testing direction exists, but final proof threshold is inconsistent with scope. |
| Dependencies / integration points | Partial | INT-02, INT-03, INT-04, REAL-03, REAL-06 | Core seams are mapped, but not fully resolved in proposal text. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P029` should be judged against current stable refs plus current HEAD, not against the original R1 framing.
- ASSUMP-02: No hidden provider-remediation or MCP-registry framework exists outside the checked-in repo.
- QUESTION-01: Is `P029` intentionally one proposal for all three executable transports, or should only the structural slice stay here with Codex/Auggie/Junie execution split later?
- QUESTION-02: Are Auggie and Junie meant to ship with any MCP lanes in this proposal, or is zero-MCP-only behavior the intended rollout state?
- QUESTION-03: If second-wave runtimes need registry validation, what is the canonical registry source per adapter family?
- BLOCKER-01: None at the evidence gate level. The remaining issues are proposal-text readiness gaps, not missing local evidence.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass.
