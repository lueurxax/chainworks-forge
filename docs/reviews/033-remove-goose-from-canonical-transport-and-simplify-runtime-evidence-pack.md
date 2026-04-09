# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-09 | High | Current `HEAD` proposal now includes a concrete proof lane, durable migration section `3.6a`, expanded docs table, and explicit brand-scope exclusion. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-09 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable when its prerequisite is still blocked. | External dependency status. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the baseline vocabulary rewrite explicitly. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current stable baseline still routes runtime/provider truth through Goose-bearing refs and old provider vocabulary. | Proposal docs migration can remain incomplete if judged only against the local table. | Current host-system truth. |
| DOC-05 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Durable provider settings, settings transfer, first-run, readiness, and Goose remediation are stable provider-platform truth today. | Proposal must cover full persisted-provider migration, not only enum values. | Provider-platform baseline. |
| DOC-06 | `docs/reference/provider-binding-truth.md` | 2026-04-09 | High | Historical run surfaces read frozen provider/model truth from persisted snapshots. | Proposal must keep historical truth legible while blocking resume. | Historical binding baseline. |
| DOC-07 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Current runtime transport is ACP-shaped but still includes Goose compatibility as current truth. | Proposal must own all canonical transport-doc fallout, not only a subset. | Runtime transport baseline. |
| DOC-08 | `docs/reference/test-gates.md` | 2026-04-09 | High | The repository still has no actual `proposal-033` lane today. | Proposal text alone is not yet repository proof ownership. | Verification baseline. |
| DOC-09 | `docs/reference/README.md` | 2026-04-09 | High | Reference index still exposes Goose transport and Goose remediation as authoritative. | Proposal must update the broader reference stack, not only isolated docs. | Reference-layer migration. |
| DOC-10 | `docs/reference/chainworks_forge_design_kit_v1.md` | 2026-04-09 | High | Design authority intentionally uses geese as brand metaphor. | Goal wording must stay scoped to runtime references, not branding. | Scope sanity check. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review intake and reusable assumptions | 2026-04-09 | High | Still fresh as entry point; affected provider/runtime surfaces needed targeted code refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem map and provider/runtime boundary | 2026-04-09 | High | Fresh overall; Goose-bearing refs remain baseline truth today. | Current system map. |
| BASE-03 | stable provider/runtime/reference docs | Partially refreshed | provider platform, transport, reference index, gate ownership, historical run truth | 2026-04-09 | High | Stable docs were sufficient, but direct code inspection was needed for persisted settings semantics. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-09 | High | No `033...review/integration-context.md` exists. Not blocking because stable refs plus targeted code refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Reused | prerequisite readiness | 2026-04-09 | High | Fresh enough to enforce the explicit prerequisite hold. | Dependency check. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - complete Goose runtime removal
  - ACP-only transport / session / executor / provider runtime architecture
  - durable settings migration for provider/platform state
  - historical Goose-run blocking and trust fallback
  - stable-reference migration and proof-gate ownership
- Out of scope:
  - completing `P030`
  - converting old Goose runs into ACP runs
  - runtime-heavy proof during this review round
- Deferred intentionally:
  - none beyond the explicit `P030` prerequisite hold
- Assumptions:
  - `P033` is a delta over current stable refs and current `HEAD`
  - the request is for `proposal-readiness`, not implementation audit
- Open questions:
  - what exact semantic rewrite happens to migrated `ConfiguredProvider` fields beyond `family` / `transport`?
  - what is the post-migration operator outcome for deleted Goose Codex rows?
  - which additional authoritative Goose-bearing refs are rewritten directly versus transitively superseded?
- Proposal-first blockers:
  - full semantic provider-row migration is still unspecified
  - docs/reference migration still omits some authoritative Goose-bearing refs
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Baseline + targeted refresh | 2026-04-09 | High | Settings is the main durable owner surface for configured providers. | Proposal could under-specify what operators see after migration. | Main operator settings surface. |
| NAV-02 | `FirstRunSetupWizard` | Baseline + targeted refresh | 2026-04-09 | High | First-run bootstrap still depends on provider seeded defaults and provider availability. | Proposal must clarify post-migration operator setup when Goose Codex rows are removed. | Bootstrap surface. |
| NAV-03 | `PilotReadinessView` | Baseline + targeted refresh | 2026-04-09 | High | Readiness remains a stable provider/remediation truth surface today. | Proposal docs migration must cover the remaining remediation references. | Provider health surface. |
| NAV-04 | `RunsHomeView` runtime provenance badge | Targeted refresh | 2026-04-09 | High | Historical trust values still render legacy Goose labels. | Proposal must keep historical-read compatibility while removing live Goose runtime. | Run-history surface. |
| NAV-05 | settings export/import flow | Baseline + targeted refresh | 2026-04-09 | High | Settings export/import persists durable provider rows exactly as configured. | Proposal can leave machine-to-machine continuity ambiguous if field semantics are not migrated explicitly. | Hidden operator-critical flow. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider model | durable provider row shape | 2026-04-09 | High | `ConfiguredProvider` persists `displayName`, `transport`, `endpoint`, `authMode`, `capabilities`, and `adapterVersion`, not only family raw values. | Proposal can leave persisted providers semantically invalid after migration. | Main migration blocker. |
| MAP-02 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Durable settings | persisted provider settings load/seed/migrate path | 2026-04-09 | High | Current seeded providers are Goose-shaped rows with Goose labels, endpoints, and auth modes. | Raw-value-only migration is insufficient to describe final operator state. | Main migration owner. |
| MAP-03 | `Chainworks Forge/Support/SettingsTransferService.swift` | Settings transfer | import/export of persisted provider settings | 2026-04-09 | High | `chainworks-settings.json` imports/exports provider settings directly. | Proposal must align transfer semantics with migration semantics. | Cross-machine durability boundary. |
| MAP-04 | `Chainworks Forge/Providers/BackendProfileResolverV2.swift` | Provider resolution | YAML/provider identifier resolution into frozen bindings | 2026-04-09 | High | Current resolver freezes provider identifiers into `ResolvedProviderBinding` and still defaults missing runtime profile to Goose. | Proposal must fully own the new provider vocabulary and resulting operator behavior. | Runtime/provider boundary. |
| MAP-05 | `docs/reference/*`, `.review-baselines/current-system-baseline.md` | Stable docs | authoritative runtime/provider/test truth | 2026-04-09 | High | Several authoritative refs outside the proposal table still carry Goose runtime truth today. | Implementation could finish with part of the reference layer stale. | Docs-layer blocker. |
| MAP-06 | `scripts/test-gate.sh`, `docs/reference/test-gates.md` | Verification | repository proof-lane ownership | 2026-04-09 | High | Proposal now describes a gate, but the repository does not yet contain it. | Proposal proof ownership is improved but not yet repo-real. | Verification baseline. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Durable provider rows | `ConfiguredProvider.swift`, `ProviderSettingsStore.swift` | persisted app state | 2026-04-09 | High | Persisted provider rows include Goose-specific semantic fields, not only enum raw values. | Proposal cannot stop at raw-value rewrite. | Primary blocking contract. |
| DATA-02 | Settings transfer package | `SettingsTransferService.swift`, `provider-platform.md` | export/import | 2026-04-09 | High | Transfer/import path reuses the same provider settings model. | Proposal must explicitly say how migration applies to transfer packages too. | Cross-machine continuity. |
| DATA-03 | Historical run truth | `Run.swift`, `provider-binding-truth.md`, `RunReportBuilder.swift`, `RunComparisonService.swift` | persisted historical read path | 2026-04-09 | High | Historical run surfaces read frozen strings and trust values directly. | Proposal’s trust fallback is directionally sufficient; this is no longer the main blocker. | Historical compatibility boundary. |
| DATA-04 | Baseline provider vocabulary | `.review-baselines/current-system-baseline.md`, `current-system-baseline.md` | baseline and review assumptions | 2026-04-09 | High | Current baseline still assumes `codex / claude_code / gemini` as MVP provider families. | Proposal must fully own the vocabulary rewrite across baseline docs. | Vocabulary boundary. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/platform durability | Baseline + current repo | 2026-04-09 | High | Provider settings and settings transfer are stable persisted surfaces today. | Proposal raw-value tables do not yet fully describe post-migration provider behavior. | Main architecture blocker. |
| INT-02 | Stable reference stack | Baseline + current repo | 2026-04-09 | High | Goose still appears in authoritative runtime/provider/test references beyond the current proposal table. | Proposal docs migration can still finish under-scoped. | Docs-layer blocker. |
| INT-03 | Historical trust readers | Current repo | 2026-04-09 | High | Historical Goose trust values still render in run-centric surfaces. | Proposal already owns trust fallback; this is no longer the main finding. | Historical-read boundary. |
| INT-04 | Repository gate ownership | Current repo | 2026-04-09 | High | `proposal-033` is not yet a repo-owned gate lane. | Proposal text is improved, but proof ownership is still future-state until implemented. | Verification boundary. |
| INT-05 | Brand/design authority | Stable docs | 2026-04-09 | High | Geese remain intentional brand metaphor in design docs. | Proposal scope wording is now correctly narrowed and no longer a blocker. | Scope sanity check. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02, NAV-03 | settings, first-run, readiness | Major entry surfaces are now explicitly owned. |
| Happy path | Partial | DOC-01, MAP-01, MAP-02, DATA-01, DATA-02 | provider migration and provider availability | Direction is clear, but semantic provider-row migration is still partial. |
| Loading | Partial | DOC-01, NAV-03 | readiness and provider diagnostics | Not a primary blocker. |
| Empty | Partial | DOC-01, NAV-01, NAV-02 | no-configured-provider and first-run empty states | Codex post-migration operator outcome remains under-specified. |
| Validation error | Partial | DOC-01, MAP-03, DATA-02 | YAML/provider migration, import validation | New provider vocabulary is specified, but some semantic migration outcomes remain open. |
| Backend error | Specified | DOC-01, MAP-04 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01, NAV-03 | readiness and troubleshooting | Secondary to the main blockers. |
| Retry / recovery | Deferred intentionally | DOC-01, DATA-03 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Auth / permission expiry | Partial | DOC-01, MAP-01, MAP-02 | provider auth mode and secrets | Migration does not yet explicitly say how stale Goose auth state is cleared or rewritten. |
| Rollback / cancellation | Partial | DOC-01, DOC-02 | implementation hold behind `P030` | External hold is explicit. |
| Historical-read compatibility | Specified | DOC-01, DATA-03, INT-03 | run history, reports, comparison | This area is substantially improved and no longer a primary blocker. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-09 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | migration described, but semantic row outcome remains partial | not yet fully specified | 2026-04-09 | High | Main rollout gap now. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-09 | High | Proposal text now defines the intended gate shape; repository ownership still awaits implementation. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-09 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and settings transfer | proposal now names `Proposal033Tests`, but semantic provider-row rewrite is still not fully specified | extend proof expectations to field-level provider migration and Codex replacement/remediation path | 2026-04-09 | High | Main proof gap. |
| TEST-03 | docs/reference migration proof | authoritative reference cleanup | proposal now names docs, but not all authoritative Goose-bearing refs are classified | extend docs matrix and proof ownership accordingly | 2026-04-09 | Medium | Remaining docs-layer gap. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | durable settings migration | migration tables make provider rewrite safe | current persisted provider rows include more Goose-shaped semantics than the tables cover | 2026-04-09 | High | Proposal still needs field-level migration closure. |
| REAL-02 | docs/reference migration | proposal docs table covers the rewrite | authoritative Goose-bearing refs still extend beyond the table | 2026-04-09 | High | Proposal docs closure is better, but still incomplete. |
| REAL-03 | stale previous findings | migration/docs/gate were missing | current proposal now includes those sections explicitly | 2026-04-09 | High | Previous three findings are obsolete and should not be reused. |
| REAL-04 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-09 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01, DOC-10 | Scope is now correctly narrowed to runtime references, not branding. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-03, DOC-04 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Provider migration states remain partial. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-04, REAL-01 | Durable provider-row semantics remain under-specified. |
| Persistence / caching | Partial | DATA-01, DATA-02, DATA-03 | Historical truth is improved; settings migration still needs closure. |
| Permissions / auth expiry | Partial | DATA-01, H table | Proposal does not yet explicitly say how stale Goose auth state is rewritten. |
| Feature flags / rollout / rollback | Partial | FLAG-01, FLAG-02 | External hold is explicit; internal migration rollout is still partial. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is now materially better in the proposal text. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Proof lane exists conceptually, but semantic migration and docs coverage still need closure. |
| Dependencies / integration points | Partial | DOC-02, REAL-04 | External prerequisite is explicit but unresolved. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current provider/runtime baseline, not only rename types and files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: what exact replacement/remediation path becomes canonical after deleting old Goose Codex provider rows?
- QUESTION-02: which `ConfiguredProvider` fields are cleared, rewritten, or preserved during migration?
- QUESTION-03: which authoritative Goose-bearing refs are rewritten directly versus transitively superseded?
- BLOCKER-01: field-level provider-row migration is still not explicit.
- BLOCKER-02: docs/reference migration still omits some authoritative Goose-bearing refs.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
