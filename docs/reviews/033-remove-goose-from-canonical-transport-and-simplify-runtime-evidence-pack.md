# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-10 | High | Current `HEAD` proposal now includes a concrete proof lane, durable migration section `3.6a`, expanded docs table, and explicit brand-scope exclusion. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-10 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable today when its prerequisite is still blocked. | External dependency status. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-10 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the baseline vocabulary rewrite explicitly. | Reusable baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-10 | High | Current stable baseline still routes runtime/provider/test truth through a broader Goose-bearing reference set than the current `3.9` table. | Proposal docs migration can remain incomplete if judged only against its local table. | Current host-system truth. |
| DOC-05 | `docs/reference/provider-platform.md` | 2026-04-10 | High | Durable provider settings and settings transfer are still stable provider-platform truth today. | Proposal proof must cover imported settings packages as well as local store migration. | Provider-platform baseline. |
| DOC-06 | `docs/reference/provider-binding-truth.md` | 2026-04-10 | High | Historical binding truth still references Goose-default execution and Goose remediation in cross-doc links. | Proposal docs migration can leave canonical provenance docs stale. | Historical binding baseline. |
| DOC-07 | `docs/reference/acp-runtime-transport.md` | 2026-04-10 | High | Current runtime transport is ACP-shaped but still includes Goose compatibility as current truth. | Proposal must own all canonical transport-doc fallout, not only a subset. | Runtime transport baseline. |
| DOC-08 | `docs/reference/run-control.md` | 2026-04-10 | High | Stable stop/cancel truth still refers to managed Goose sessions. | Proposal docs migration can miss a stable operator/runtime contract. | Control-plane baseline. |
| DOC-09 | `docs/reference/skill-resolution-and-runtime-integration.md` | 2026-04-10 | High | Stable skill/runtime doc still routes execution through `GooseSessionBridge / runtime execution packet`. | Proposal can leave skill/runtime integration docs stale while claiming transport cleanup is complete. | Skill/runtime reference layer. |
| DOC-10 | `docs/reference/test-suite-architecture.md` | 2026-04-10 | High | Stable suite architecture still enumerates Goose fixtures and Goose test classes as current truth. | Proposal can leave canonical testing docs stale after fixture migration. | Test baseline. |
| DOC-11 | `docs/reference/test-gates.md` | 2026-04-10 | High | Repository still has no actual `proposal-033` lane today, but the proposal now owns a concrete lane shape. | Proposal proof ownership can be misjudged as missing when it is now proposal-owned but unimplemented. | Verification baseline. |
| DOC-12 | `docs/reference/agent-ui-test-execution.md` | 2026-04-10 | High | Canonical UI-proof doc still names Goose assistant/runtime proof surfaces and links Goose remediation. | Proposal docs migration can miss part of the canonical proof stack. | UI verification baseline. |
| DOC-13 | `docs/reference/README.md` | 2026-04-10 | High | Reference index still exposes Goose transport and Goose remediation as authoritative. | Proposal must update the broader reference stack, not only isolated docs. | Reference-layer migration. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review intake and reusable assumptions | 2026-04-10 | High | Still fresh as entry point; affected provider/runtime/doc surfaces needed targeted refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem map and provider/runtime boundary | 2026-04-10 | High | Fresh overall; Goose-bearing refs remain baseline truth today. | Current system map. |
| BASE-03 | stable provider/runtime/reference docs | Partially refreshed | provider platform, binding truth, run control, skill/runtime docs, test docs, reference index, gate ownership | 2026-04-10 | High | Stable docs were sufficient; targeted refresh was needed to check which authoritative Goose-bearing refs still exceed the proposal's table. | Narrow baseline refresh. |
| BASE-04 | proposal-specific integration context | Missing | none | 2026-04-10 | High | No `033...review/integration-context.md` exists. Not blocking because stable refs plus targeted refresh were enough. | None blocking. |
| BASE-05 | adjacent implementation audit for `P030` | Reused | prerequisite readiness | 2026-04-10 | High | Fresh enough to enforce the explicit prerequisite hold. | Dependency check. |

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
- Proposal-first blockers:
  - the docs/reference migration table still omits some baseline-authoritative Goose-bearing docs
  - the proof lane still does not explicitly prove `SettingsTransferService` import migration
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ProviderSettingsView` | Baseline + targeted refresh | 2026-04-10 | High | Settings is the main durable owner surface for configured providers and transfer continuity. | Proposal proof can under-specify cross-machine migration if judged only against local store migration. | Main operator settings surface. |
| NAV-02 | `FirstRunSetupWizard` | Baseline + targeted refresh | 2026-04-10 | High | First-run bootstrap still depends on provider seeded defaults and provider availability. | Not a blocker now; Codex replacement path is explicit in `3.6a`. | Bootstrap surface. |
| NAV-03 | `PilotReadinessView` | Baseline + targeted refresh | 2026-04-10 | High | Readiness remains a stable provider/remediation truth surface today. | Proposal docs migration must cover the remaining remediation-linked references. | Provider health surface. |
| NAV-04 | `RunsHomeView` runtime provenance badge | Targeted refresh | 2026-04-10 | High | Historical trust values still render legacy Goose labels. | Proposal trust fallback remains adequate; no longer a main blocker. | Run-history surface. |
| NAV-05 | settings export/import flow | Baseline + targeted refresh | 2026-04-10 | High | `chainworks-settings.json` is a durable operator flow distinct from local provider-settings load. | Proof lane can miss a critical migration surface if it only proves `ProviderSettingsStore`. | Hidden operator-critical flow. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ConfiguredProvider.swift` | Provider model | durable provider row shape | 2026-04-10 | High | `ConfiguredProvider` persists family, display name, transport, endpoint, auth mode, capabilities, and adapter version. | Migration proof must protect more than raw enum values. | Migration scope anchor. |
| MAP-02 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Durable settings | persisted provider settings load/seed/migrate path | 2026-04-10 | High | Current load path still decodes plain `ProviderSettings`; proposal now explicitly owns a one-time Goose-era migration. | Proposal proof must cover this local-store migration. | Local durability boundary. |
| MAP-03 | `Chainworks Forge/Support/SettingsTransferService.swift` | Settings transfer | import/export of persisted provider settings | 2026-04-10 | High | `chainworks-settings.json` imports/exports provider settings directly and validates provider families before merge. | Proposal proof can be incomplete if transfer-package migration is not explicitly covered. | Cross-machine durability boundary. |
| MAP-04 | `docs/reference/*`, `.review-baselines/current-system-baseline.md` | Stable docs | authoritative runtime/provider/test truth | 2026-04-10 | High | Several authoritative refs outside the proposal table still carry Goose runtime truth today. | Implementation could finish with part of the reference layer stale. | Docs-layer blocker. |
| MAP-05 | `scripts/test-gate.sh`, `docs/reference/test-gates.md` | Verification | repository proof-lane ownership | 2026-04-10 | High | Proposal now describes a concrete `proposal-033` lane shape, but the repo does not yet contain it. | Proposal proof ownership should be judged against proposal text in this mode, not current implementation. | Verification baseline. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Durable provider rows | `ConfiguredProvider.swift`, `ProviderSettingsStore.swift`, `provider-platform.md` | persisted local settings | 2026-04-10 | High | Proposal now explicitly covers field-level migration semantics for Goose-era provider rows. | Previous raw-value-only finding is stale. | Local migration contract. |
| DATA-02 | Settings transfer package | `SettingsTransferService.swift`, `provider-platform.md` | exported/imported machine state | 2026-04-10 | High | Transfer packages are a second durable migration surface beyond `provider-settings.json`. | Proposal proof can miss cross-machine continuity if not named explicitly. | Cross-machine continuity. |
| DATA-03 | Historical run truth | `Run.swift`, `provider-binding-truth.md`, `RunReportBuilder.swift`, `RunComparisonService.swift` | persisted historical read path | 2026-04-10 | High | Historical run surfaces read frozen strings and trust values directly. | Proposal's trust fallback is directionally sufficient; this is no longer the main blocker. | Historical compatibility boundary. |
| DATA-04 | Baseline provider vocabulary | `.review-baselines/current-system-baseline.md`, `current-system-baseline.md` | baseline and review assumptions | 2026-04-10 | High | Current baseline still assumes `codex / claude_code / gemini` as MVP provider families. | Proposal must fully own the vocabulary rewrite across baseline docs. | Vocabulary boundary. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Provider/platform durability | Baseline + current repo | 2026-04-10 | High | Provider settings and settings transfer are stable persisted surfaces today. | Proposal now covers both, but proof only names one explicitly. | Proof-scope gap. |
| INT-02 | Stable reference stack | Baseline + current repo | 2026-04-10 | High | Goose still appears in authoritative runtime/provider/test references beyond the current proposal table. | Proposal docs migration can still finish under-scoped. | Docs-layer blocker. |
| INT-03 | Historical trust readers | Current repo | 2026-04-10 | High | Historical Goose trust values still render in run-centric surfaces. | Proposal already owns trust fallback; this is no longer the main finding. | Historical-read boundary. |
| INT-04 | Repository gate ownership | Current repo | 2026-04-10 | High | `proposal-033` is not yet repo-real, but the proposal now defines its intended shape concretely. | This is no longer a proposal-text blocker by itself. | Verification boundary. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02, NAV-03 | settings, first-run, readiness | Major entry surfaces are explicitly owned. |
| Happy path | Specified | DOC-01, MAP-01, MAP-02, DATA-01 | provider migration and provider availability | Field-level provider-row migration is now explicit. |
| Loading | Partial | DOC-01, NAV-03 | readiness and provider diagnostics | Not a primary blocker. |
| Empty | Specified | DOC-01, NAV-01, NAV-02 | no-configured-provider and first-run empty states | Codex replacement path is explicit enough now. |
| Validation error | Partial | DOC-01, MAP-03, DATA-02 | settings transfer import validation | Transfer-package migration proof remains under-specified. |
| Backend error | Specified | DOC-01, MAP-04 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01, NAV-03 | readiness and troubleshooting | Secondary to the main blockers. |
| Retry / recovery | Deferred intentionally | DOC-01, DATA-03 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Auth / permission expiry | Specified | DOC-01, MAP-01 | provider auth mode migration | Proposal now explicitly keeps or clears relevant auth state. |
| Rollback / cancellation | Partial | DOC-01, DOC-02 | implementation hold behind `P030` | External hold is explicit. |
| Historical-read compatibility | Specified | DOC-01, DATA-03, INT-03 | run history, reports, comparison | This area is substantially improved and no longer a primary blocker. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-10 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | migration contract is now explicit | no rollback path beyond hold/retry on same tree | 2026-04-10 | High | Main remaining gap is proof coverage, not migration semantics. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-10 | High | Proposal text now defines the intended gate shape; remaining gap is transfer-migration proof scope. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-10 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and settings transfer | proposal now names `Proposal033Tests`, but the explicit proof list covers only `ProviderSettingsStore.migrateFromGooseEra()` | extend proof expectations to `SettingsTransferService` import migration too | 2026-04-10 | High | Main proof gap. |
| TEST-03 | docs/reference migration proof | authoritative reference cleanup | proposal now names many docs, but not the full baseline-authoritative Goose-bearing set | extend docs matrix and proof ownership accordingly | 2026-04-10 | High | Remaining docs-layer gap. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | docs/reference migration | proposal docs table covers the rewrite | baseline-authoritative Goose-bearing refs still extend beyond the current table | 2026-04-10 | High | Proposal docs closure is much better, but still incomplete. |
| REAL-02 | proof lane vs migration scope | `3.6a` covers local + transfer migration | explicit proof list only names local-store migration | 2026-04-10 | High | Proposal proof contract is narrower than migration scope. |
| REAL-03 | stale previous findings | migration/docs/gate were missing | current proposal now includes those sections explicitly | 2026-04-10 | High | Previous three findings are obsolete and should not be reused. |
| REAL-04 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-10 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | Scope is correctly narrowed to runtime references, not branding. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-03, DOC-04 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Validation / transfer proof remains partial. |
| Data / API contract | Specified | DATA-01, DATA-02 | Durable migration semantics are now explicit enough. |
| Persistence / caching | Partial | DATA-02, REAL-02 | Transfer-package proof remains under-specified. |
| Permissions / auth expiry | Specified | DATA-01 | Auth handling is explicit enough now. |
| Feature flags / rollout / rollback | Partial | FLAG-01, FLAG-02 | External hold is explicit; transfer proof remains partial. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is materially better in the proposal text. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Proof lane exists, but transfer migration and docs coverage still need closure. |
| Dependencies / integration points | Specified | DOC-02, REAL-04 | External prerequisite is explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current provider/runtime baseline, not only rename types and files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: should the omitted baseline-authoritative docs be rewritten directly in `P033`, or explicitly marked as transitively superseded?
- QUESTION-02: should the transfer-migration proof live inside `Proposal033Tests`, `ProviderPlatformTests`, or both?
- BLOCKER-01: docs/reference migration still omits some baseline-authoritative Goose-bearing docs.
- BLOCKER-02: proof lane still omits explicit `SettingsTransferService` import migration proof.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
