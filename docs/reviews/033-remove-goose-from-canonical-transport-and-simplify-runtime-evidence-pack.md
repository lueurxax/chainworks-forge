# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-09 | High | `P033` is now a full Goose-removal / ACP-only architecture proposal, not a compatibility-only cleanup. | Review could keep stale findings from the previous concept. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-09 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as immediately executable when its prerequisite is still blocked. | External dependency status. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-09 | High | Review intake still starts from a Goose-bearing current-system baseline and current MVP provider boundary. | Proposal could be judged as if the repo baseline had already migrated. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-09 | High | Current baseline still treats live Goose-backed execution, Goose compatibility transport, and Goose remediation as stable system truth. | Proposal could under-own the migration fallout across baseline docs and operator surfaces. | Current host-system truth. |
| DOC-05 | `docs/reference/provider-platform.md` | 2026-04-09 | High | Durable provider settings, settings transfer, first-run, readiness, and Goose remediation are current stable provider-platform truth. | Proposal could miss durable config and operator migration owners. | Provider-platform baseline. |
| DOC-06 | `docs/reference/provider-binding-truth.md` | 2026-04-09 | High | Frozen provider/model truth remains authoritative for historical run surfaces. | Proposal could oversimplify the reader side of provider-truth migration. | Historical binding baseline. |
| DOC-07 | `docs/reference/acp-runtime-transport.md` | 2026-04-09 | High | Current runtime transport is ACP-shaped but still includes Goose compatibility and Goose-default fallback behavior. | Proposal could overstate what is already ACP-only. | Runtime transport baseline. |
| DOC-08 | `docs/reference/test-gates.md` | 2026-04-09 | High | The repository currently has no `proposal-033` gate. | Proposal verification could remain purely conceptual. | Verification ownership. |
| DOC-09 | `docs/reference/README.md` | 2026-04-09 | High | Reference index still exposes Goose transport and Goose remediation as authoritative live docs. | Proposal could leave the primary reference index stale. | Reference-layer migration. |
| DOC-10 | `docs/reference/chainworks_forge_design_kit_v1.md` | 2026-04-09 | High | The product brand intentionally uses geese as a visual metaphor. | Proposal goal wording could overreach beyond runtime/transport scope. | Goal-scope sanity check. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level review intake and current provider boundary | 2026-04-09 | High | Still fresh as review entry point, but affected provider/runtime surfaces needed targeted code refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | implemented system map and canonical stable refs | 2026-04-09 | High | Fresh overall; `P033` touches many stable refs beneath it. | Current system map. |
| BASE-03 | stable runtime/provider/reference docs | Partially refreshed | provider platform, transport, provider truth, reference index, test-gate ownership | 2026-04-09 | High | Stable docs were sufficient, but direct code inspection was needed for durable settings and gate ownership. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-09 | High | No `033...review/integration-context.md` exists. Not blocking because stable refs plus targeted code refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Reused | prerequisite readiness | 2026-04-09 | High | Fresh enough to enforce the external dependency gate. | Dependency check. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - remove Goose from runtime transport, session, executor, configuration, provider-platform, fixture, UI, and operator-remediation layers
  - make ACP the only live runtime path
  - block historical Goose-bound runs from resume
  - preserve historical trust readability without converting old run data
- Out of scope:
  - completing `P030`
  - converting Goose runs into ACP runs
  - runtime-heavy proof during this review round
- Deferred intentionally:
  - none beyond the explicit `P030` prerequisite hold
- Assumptions:
  - `P033` is a delta over current stable refs and current `HEAD`
  - the request is for `proposal-readiness`, not implementation audit or product prioritization
- Open questions:
  - what are the canonical post-`P033` provider identifiers in YAML and settings?
  - how do `provider-settings.json` and `chainworks-settings.json` migrate?
  - which Goose-bearing docs are kept intentionally because they are branding or historical evidence, not runtime truth?
  - what exact suites and outputs make up `proposal-033`?
- Proposal-first blockers:
  - no durable provider-platform migration contract
  - docs/reference migration inventory is under-scoped
  - `proposal-033` gate is still conceptual
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Baseline + targeted refresh | 2026-04-09 | High | Settings is a durable owner surface for configured providers and transport/family labels. | Proposal could miss the persisted-settings migration impact. | Main operator settings surface. |
| NAV-02 | `FirstRunSetupWizard` | Baseline + targeted refresh | 2026-04-09 | High | First-run bootstrap still depends on provider-platform seeded defaults and settings transfer assumptions. | Proposal could remove Goose paths without defining ACP-only bootstrap migration. | Bootstrap surface. |
| NAV-03 | `PilotReadinessView` | Baseline + targeted refresh | 2026-04-09 | High | Readiness remains a stable provider/remediation truth surface today. | Proposal must rewrite it coherently when Goose disappears. | Provider health surface. |
| NAV-04 | `RunsHomeView` runtime provenance badge | Targeted refresh | 2026-04-09 | High | Historical trust values still render Goose-specific labels. | Proposal must preserve historical legibility while changing current runtime vocabulary. | Run-history surface. |
| NAV-05 | settings export/import flow | Baseline + targeted refresh | 2026-04-09 | High | Settings export/import persists durable provider settings and family keys today. | Proposal can break cross-machine continuity without an explicit migration contract. | Hidden but critical operator flow. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider model | durable provider family / transport vocabulary | 2026-04-09 | High | `ConfiguredProvider`, `ProviderFamily`, and `ProviderTransport` still persist Goose-era enum/raw values and runtime identifiers. | Proposal can under-specify durable migration. | Main schema boundary. |
| MAP-02 | `Chainworks Forge/Providers/ProviderSettings.swift`, `Chainworks Forge/Providers/ProviderSettingsStore.swift`, `Chainworks Forge/Support/SettingsTransferService.swift` | Durable settings | persisted provider settings and export/import | 2026-04-09 | High | Current provider settings store and transfer package serialize configured providers and preferred-provider keys directly. | Proposal can strand user settings if migration is implicit. | Main migration blocker. |
| MAP-03 | `Chainworks Forge/Providers/BackendProfileResolverV2.swift` | Provider resolution | canonical provider family / runtime profile resolution | 2026-04-09 | High | Current resolver still defaults missing runtime profiles to legacy Goose and freezes provider identifiers into `ResolvedProviderBinding`. | Proposal must explicitly redefine the post-Goose provider vocabulary. | Runtime/provider boundary. |
| MAP-04 | `Chainworks Forge/Engine/ExecutionService.swift` | Runtime transport factory | current Goose transport injection and fixture selection | 2026-04-09 | High | Current live executor still injects shared Goose transport and Goose fixtures. | Proposal scope is real and broad, not theoretical. | Transport inventory anchor. |
| MAP-05 | `Chainworks Forge/scripts/test-gate.sh`, `docs/reference/test-gates.md` | Verification | repository-owned proof-lane ownership | 2026-04-09 | High | No `proposal-033` lane exists today. | Proposal verification can remain non-operational. | Proof-gate blocker. |
| MAP-06 | `docs/reference/*`, `.review-baselines/current-system-baseline.md` | Stable docs | authoritative runtime/provider/troubleshooting truth | 2026-04-09 | High | Goose remains embedded across many authoritative docs, far beyond the four docs named in `P033`. | Baseline can remain self-contradictory after implementation. | Docs-layer blocker. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Durable provider settings | `ConfiguredProvider.swift`, `ProviderSettings.swift`, `ProviderSettingsStore.swift` | persisted app state | 2026-04-09 | High | Provider family and transport are persisted as durable config, not transient UI state. | Proposal cannot safely rename/delete them without migration. | Primary blocking contract. |
| DATA-02 | Settings transfer package | `SettingsTransferService.swift`, `provider-platform.md` | export/import | 2026-04-09 | High | `chainworks-settings.json` directly embeds `ProviderSettings`. | Proposal needs schema/version migration, not only in-memory refactor steps. | Cross-machine continuity. |
| DATA-03 | Frozen provider bindings | `Run.swift`, `provider-binding-truth.md`, `RunReportBuilder.swift`, `RunComparisonService.swift` | historical run truth | 2026-04-09 | High | Historical runs read frozen provider bindings and provenance from persisted snapshots. | Proposal must keep reader-side historical truth legible while blocking resume. | Historical compatibility surface. |
| DATA-04 | Provider/runtime identifier baseline | `.review-baselines/current-system-baseline.md`, `current-system-baseline.md`, `ConfiguredProvider.swift` | baseline + runtime mapping | 2026-04-09 | High | Current canonical provider identifiers remain `codex`, `claude_code`, `gemini`. | Proposal must explicitly state whether identifiers change. | Vocabulary boundary. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/platform durability | Baseline + current repo | 2026-04-09 | High | Provider settings and settings transfer are stable persisted surfaces today. | Proposal still treats the provider rewrite like a pure code refactor. | Main architecture blocker. |
| INT-02 | Stable reference stack | Baseline + current repo | 2026-04-09 | High | Goose is still part of the authoritative reference map and baseline intake. | Proposal docs layer is too narrow for the actual baseline fallout. | Docs-layer blocker. |
| INT-03 | Historical trust readers | Current repo | 2026-04-09 | High | `RunsHomeView` still renders legacy Goose trust labels; frozen bindings remain reader truth elsewhere. | Proposal must separate current-runtime cleanup from historical-read compatibility. | Reader compatibility boundary. |
| INT-04 | Repository test-gate ownership | Current repo | 2026-04-09 | High | Current repo has no `proposal-033` gate lane. | Proposal acceptance remains non-operational. | Verification blocker. |
| INT-05 | Brand/design authority | Stable docs | 2026-04-09 | High | The product brand intentionally uses geese as a metaphor. | Proposal goal wording can overreach beyond runtime/transport scope. | Goal-scope sanity check. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02, NAV-03 | settings, first-run, readiness | Major entry surfaces are named. |
| Happy path | Partial | DOC-01, MAP-01, MAP-02, DATA-01, DATA-02 | provider settings, settings transfer, runtime resolution | End-state direction is clear, but persisted migration is not. |
| Loading | Partial | DOC-01, NAV-03 | readiness and provider diagnostics | Not the main blocker, but migration proof still needs verification ownership. |
| Empty | Partial | DOC-01, NAV-01, NAV-02 | no-configured-provider and first-run empty states | ACP-only bootstrap semantics are not fully specified. |
| Validation error | Partial | DOC-01, MAP-02, DATA-02 | imported settings, unsupported provider families, bad config | Durable migration and validation behavior remain partial. |
| Backend error | Partial | DOC-01, MAP-04 | runtime transport removal / missing ACP binaries | Runtime failure direction exists, but proof lane is not defined. |
| Offline / degraded | Partial | DOC-01, NAV-03 | readiness and troubleshooting | Proposal direction exists, but operator proof is not operationalized. |
| Retry / recovery | Deferred intentionally | DOC-01, DATA-03 | historical Goose runs blocked rather than migrated | Acceptable deferral; proposal explicitly rejects conversion. |
| Auth / permission expiry | Deferred intentionally | DOC-01, DOC-05 | provider auth and keychain | Not central to the current blockers. |
| Rollback / cancellation | Partial | DOC-01, DOC-02 | implementation hold behind `P030` | External dependency gate is explicit, but rollback proof is not detailed. |
| Historical-read compatibility | Partial | DOC-01, DOC-06, NAV-04, DATA-03 | run history, reports, comparison | Trust-label fallback is specified, but broader provider/settings migration remains incomplete. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-09 | High | This is now explicit and correct. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | not yet specified | not yet specified | 2026-04-09 | High | Main rollout gap inside the proposal. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-09 | High | Gate is named in the proposal but not yet defined in the repo. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-09 | High | External hold remains real. |
| TEST-02 | provider/platform migration proof | settings persistence and export/import | no `P033` migration proof lane exists | add explicit proof for provider-settings migration and settings-transfer compatibility | 2026-04-09 | High | Main testing gap. |
| TEST-03 | runtime removal proof | ACP-only transport selection and Goose-run blocking | no `proposal-033` gate exists | add named suites proving transport removal, ACP-only resolution, and resume blocking | 2026-04-09 | High | Acceptance `12` is currently non-operational. |
| TEST-04 | stable-doc/gate ownership proof | docs/reference and gate updates | no repository mechanism ties doc migration to proof | add explicit evidence outputs for rewritten/deleted/retained docs | 2026-04-09 | Medium | Docs-layer fallout can remain implicit otherwise. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | provider family / transport rewrite | provider layer can be cleaned to ACP-only by deleting Goose-era families/transports | durable settings and settings transfer still serialize those exact values and keys | 2026-04-09 | High | Proposal needs an explicit migration contract, not only refactor steps. |
| REAL-02 | docs layer | four doc updates cover the migration | authoritative Goose-bearing refs exist across baseline, reference index, transport, provider, workflow, test, and remediation docs | 2026-04-09 | High | Docs/reference ownership is under-scoped. |
| REAL-03 | proof gate | `proposal-033` gate will prove the slice | no repository `proposal-033` lane exists today | 2026-04-09 | High | Acceptance remains non-operational. |
| REAL-04 | removal goal | zero Goose references in the codebase | the design authority intentionally uses geese as a brand metaphor | 2026-04-09 | High | Goal wording must be narrowed to runtime/transport scope. |
| REAL-05 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-09 | High | Operational start is still blocked even after proposal fixes. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | The simplification intent is clear. |
| Scope boundaries | Specified | DOC-01 | Full Goose removal is now explicit. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-03, DOC-04 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Primary provider/operator entry points are known. |
| State handling | Partial | H table | Historical-read fallback is only partially covered beyond trust labels. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-04, REAL-01 | Durable provider/settings migration is still unspecified. |
| Persistence / caching | Partial | DATA-01, DATA-02, DATA-03 | Historical run truth is acknowledged, but durable settings migration is not. |
| Permissions / auth expiry | Deferred intentionally | DOC-01, DOC-05 | Not a current blocker. |
| Feature flags / rollout / rollback | Partial | FLAG-01, FLAG-02 | External hold is explicit; internal migration rollout is not. |
| Analytics / instrumentation | Partial | METRIC-01 | Proof gate exists only as proposal text. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03, TEST-04 | Verification ownership is still incomplete. |
| Dependencies / integration points | Partial | DOC-02, REAL-05 | External prerequisite is explicit, but still unresolved. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to rewrite the current stable provider/runtime baseline, not only rename code files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: what exact provider identifiers survive in YAML and settings after `P033`?
- QUESTION-02: what migration or schema-version policy applies to `provider-settings.json` and `chainworks-settings.json`?
- QUESTION-03: which Goose-bearing docs are deleted, rewritten, or retained intentionally because they describe brand metaphor or historical evidence?
- QUESTION-04: what exact suites and outputs make up `proposal-033`?
- BLOCKER-01: durable provider-platform migration is not yet specified.
- BLOCKER-02: docs/reference migration inventory is still under-scoped.
- BLOCKER-03: `proposal-033` verification lane is still not operationally defined.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation still cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
