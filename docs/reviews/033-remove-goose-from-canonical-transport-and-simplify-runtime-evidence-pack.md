# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-10 | High | Current `HEAD` proposal now includes expanded docs migration and explicit `SettingsTransferService` proof. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/reference/domain-model.md` | 2026-04-10 | High | Canonical model doc still documents `AgentExecution.gooseSessionID` as durable schema truth. | Proposal can claim zero Goose Swift/runtime refs while leaving persistent model fallout implicit. | Persistent-model reference. |
| DOC-03 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-10 | High | Stable execution-truth doc still names `GooseAgentExecutor` as a current owner of classification behavior. | Proposal docs migration can remain incomplete around executor/model vocabulary. | Canonical runtime-truth reference. |
| DOC-04 | `.review-baselines/current-system-baseline.md` | 2026-04-10 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the baseline vocabulary rewrite explicitly. | Reusable baseline intake. |
| DOC-05 | `docs/reference/current-system-baseline.md` | 2026-04-10 | High | Current stable baseline still routes subsystem truth through Goose-bearing runtime docs. | Proposal cleanup must stay consistent with the broader reference stack. | Current host-system truth. |
| DOC-06 | `docs/reference/provider-platform.md` | 2026-04-10 | High | Durable provider settings and settings transfer are stable provider-platform truth today. | Earlier proof-gap finding is now stale because the proposal explicitly added transfer-path proof. | Provider-platform baseline. |
| DOC-07 | `docs/reference/README.md` | 2026-04-10 | High | Reference index still exposes Goose transport and Goose remediation as authoritative today. | Proposal docs migration must stay coherent across the public reference index. | Reference-layer migration. |
| DOC-08 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-10 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable today when its prerequisite is still blocked. | External dependency status. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review intake and reusable assumptions | 2026-04-10 | High | Still fresh as entry point; affected model/doc surfaces needed targeted refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem map and provider/runtime boundary | 2026-04-10 | High | Fresh overall; runtime docs remain Goose-bearing today. | Current system map. |
| BASE-03 | stable provider/runtime/model docs | Partially refreshed | domain model, execution truth, provider platform, reference index | 2026-04-10 | High | Stable docs were sufficient; targeted refresh was needed to inspect persistent-model fallout and operator wording contradictions. | Narrow baseline refresh. |
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
  - the proposal contradicts itself on whether Goose may still appear in operator-facing historical strings
  - the proposal does not explicitly own the persisted `gooseSessionID` field or its fallout
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | blocked historical run surface (`RunsHomeView` / recovery surfaces) | Proposal + baseline | 2026-04-10 | High | Proposal section `4` still prescribes Goose-labeled blocked-run copy. | Acceptance can become unprovable if operator wording policy is contradictory. | Historical operator surface. |
| NAV-02 | runtime trust display | Proposal + baseline | 2026-04-10 | High | Proposal trust model still displays `"Legacy ... historical Goose runs"`. | Zero-Goose operator wording rule is internally inconsistent. | Historical run-history surface. |
| NAV-03 | settings export/import flow | Baseline + proposal | 2026-04-10 | High | Proposal now explicitly owns transfer-path migration proof. | Previous proof-gap finding is stale. | Cross-machine continuity surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Models/AgentExecution.swift` | Persistent model | durable session identifier storage | 2026-04-10 | High | `gooseSessionID` is still the real persisted property; `runtimeSessionID` is only a compatibility accessor. | Proposal can claim zero Goose Swift refs without deciding whether to migrate or grandfather the persisted field. | Main model-layer blocker. |
| MAP-02 | `Chainworks Forge/Engine/SupportBundleExporter.swift` | Support export | serialized run diagnostic output | 2026-04-10 | High | Support bundle still exports `gooseSessionID` as a JSON key. | Model-layer fallout extends beyond SwiftData schema into exported diagnostics. | Secondary model/export fallout. |
| MAP-03 | `Chainworks Forge/docs/reference/domain-model.md` | Stable docs | canonical data-model description | 2026-04-10 | High | Stable docs still document `gooseSessionID` directly. | Proposal docs cleanup is incomplete around the persistent model. | Model-doc fallout. |
| MAP-04 | `Chainworks Forge/docs/reference/execution-truth-and-recovery.md` | Stable docs | canonical runtime/executor truth | 2026-04-10 | High | Stable runtime-truth docs still name `GooseAgentExecutor`. | Proposal docs cleanup is incomplete around executor vocabulary. | Runtime-doc fallout. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Durable provider rows | `ConfiguredProvider.swift`, `ProviderSettingsStore.swift`, `SettingsTransferService.swift` | persisted settings | 2026-04-10 | High | Proposal now explicitly covers local and imported settings migration. | Previous migration-proof gap is stale. | Closed area. |
| DATA-02 | Persistent session identifier | `AgentExecution.gooseSessionID`, `SupportBundleExporter`, `domain-model.md` | persisted model and export | 2026-04-10 | High | The proposal does not currently say whether this field is renamed, grandfathered, or excluded from the zero-Goose goal. | Implementers would have to invent schema compatibility behavior. | Main persistence gap. |
| DATA-03 | Historical operator wording | proposal sections `3.9`, `4`, `6`, `7` | user-visible copy contract | 2026-04-10 | High | Goal/scope/acceptance ban Goose operator strings while later sections still prescribe them. | Proposal acceptance becomes internally contradictory. | Main wording blocker. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Historical run/trust rendering | Proposal + baseline | 2026-04-10 | High | Historical legacy copy still says `Goose` in the proposal body. | Proposal cannot satisfy both zero-Goose UI wording and explicit legacy labels. | Cross-discipline blocker. |
| INT-02 | Persistent model compatibility | Current repo + stable docs | 2026-04-10 | High | Persistent runtime session storage still uses Goose-era naming. | Proposal must own a model/storage decision instead of leaving it implicit. | Architecture blocker. |
| INT-03 | Repository gate ownership | Proposal + repo | 2026-04-10 | High | `proposal-033` is still not repo-real, but the proposal now defines its intended shape and transfer-path proof. | This is no longer the main proposal-text blocker. | Verification baseline. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-03, DATA-01 | settings, migration, proof lane | Major entry surfaces are explicitly owned. |
| Happy path | Specified | DOC-01, DATA-01 | provider migration and transport cleanup | Earlier settings/proof gaps are closed. |
| Historical-read compatibility | Partial | NAV-01, NAV-02, DATA-02, DATA-03 | legacy trust, blocked Goose runs, persistent session IDs | Proposal still leaves wording and persisted model strategy inconsistent. |
| Validation error | Specified | DOC-01, DATA-01 | settings transfer import validation | Closed enough for this pass. |
| Backend error | Specified | DOC-01 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01 | readiness and troubleshooting | Secondary to the main blockers. |
| Retry / recovery | Deferred intentionally | DOC-01 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Rollback / cancellation | Partial | DOC-01, DOC-08 | implementation hold behind `P030` | External hold remains explicit. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-10 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | contract + proof now explicit | no special rollback beyond hold | 2026-04-10 | High | No longer a main blocker. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-10 | High | Proposal text now defines the intended gate shape well enough; remaining blockers are not proof-lane omissions. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-10 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and transfer packages | proposal now explicitly names both local and transfer-path proof | none for this finding set | 2026-04-10 | High | Closed enough for proposal-readiness. |
| TEST-03 | model/storage fallout proof | persisted `gooseSessionID` compatibility | no explicit proof because proposal never classifies the field | add proof once proposal chooses rename vs grandfather strategy | 2026-04-10 | High | Follows from the missing model contract. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | operator-facing wording | zero Goose runtime references in operator-facing UI strings | proposal sections `4` and `6` still prescribe Goose-labeled legacy copy | 2026-04-10 | High | Proposal is internally contradictory. |
| REAL-02 | persistent model strategy | zero Goose runtime references in Swift source | persistent model still uses `gooseSessionID`, and proposal does not classify it | 2026-04-10 | High | Proposal leaves a schema/compatibility decision implicit. |
| REAL-03 | previous docs/proof gaps | docs/proof were incomplete | current proposal now includes those sections explicitly | 2026-04-10 | High | Previous stale findings should not be reused. |
| REAL-04 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-10 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Contradicted by repo | DATA-03, REAL-01 | Zero-Goose operator wording conflicts with later sections. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05 | Local evidence is sufficient. |
| Screen / surface definition | Partial | NAV-01, NAV-02 | Historical legacy surfaces are described, but wording policy is contradictory. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Historical-read state remains partial because model/writing policy is unsettled. |
| Data / API contract | Partial | DATA-02, REAL-02 | Persistent session-field strategy is under-specified. |
| Persistence / caching | Partial | DATA-02, TEST-03 | `gooseSessionID` fate remains implicit. |
| Permissions / auth expiry | Specified | DATA-01 | Not a blocker in this pass. |
| Feature flags / rollout / rollback | Specified | FLAG-01, FLAG-02 | External hold is explicit. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is adequate for proposal-readiness. |
| Testing strategy | Partial | TEST-03 | Model/storage compatibility proof awaits a clearer contract. |
| Dependencies / integration points | Specified | DOC-08, REAL-04 | External prerequisite is explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current runtime/provider baseline, not only remove transport files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: should historical blocked-run/trust surfaces still say `Goose`, or should they switch to neutral `Legacy runtime` wording?
- QUESTION-02: is `gooseSessionID` intended to remain as a grandfathered storage alias, or should `P033` own a real persisted-model migration?
- BLOCKER-01: goal/scope/acceptance and historical operator copy are internally contradictory.
- BLOCKER-02: persistent model/storage fallout for `gooseSessionID` remains unowned.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
