# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-10 | High | Current `HEAD` proposal now includes transfer-path proof, neutral historical operator wording, and explicit `runtimeSessionID` migration. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/reference/runtime-contract.md` | 2026-04-10 | High | Stable runtime boundary still freezes MVP providers as `codex`, `claude_code`, `gemini`. | Proposal can change canonical provider identifiers without updating a stable boundary owner. | Runtime boundary reference. |
| DOC-03 | `docs/reference/mvp-sign-off.md` | 2026-04-10 | High | Stable MVP sign-off still defines the frozen provider set as `codex`, `claude_code`, `gemini`. | Proposal can leave sign-off/evaluation truth stale after provider vocabulary migration. | Sign-off boundary reference. |
| DOC-04 | `.review-baselines/current-system-baseline.md` | 2026-04-10 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the boundary rewrite explicitly, not only subset docs. | Reusable baseline intake. |
| DOC-05 | `docs/reference/current-system-baseline.md` | 2026-04-10 | High | Current stable baseline still routes subsystem truth through Goose-era provider/runtime terminology. | Proposal cleanup must stay consistent with the broader reference stack. | Current host-system truth. |
| DOC-06 | `docs/reference/domain-model.md` | 2026-04-10 | High | Canonical model doc still documents the old `gooseSessionID` field today, but the proposal now owns a concrete rename. | Previous persistent-model ownership finding is stale. | Closed model-doc area. |
| DOC-07 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-10 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable today when its prerequisite is still blocked. | External dependency status. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review intake and reusable assumptions | 2026-04-10 | High | Still fresh as entry point; affected migration/boundary surfaces needed targeted refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem map and provider/runtime boundary | 2026-04-10 | High | Fresh overall; provider/runtime boundary still uses the old provider set. | Current system map. |
| BASE-03 | stable provider/runtime/sign-off docs and policy code | Partially refreshed | runtime boundary, sign-off boundary, provider UUID/secret coupling | 2026-04-10 | High | Stable docs plus targeted code inspection were enough to isolate the remaining blockers. | Narrow baseline refresh. |
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
  - the proposal deletes old Codex rows without defining UUID/keychain continuity
  - the proposal changes canonical provider identifiers without classifying all canonical MVP boundary owners
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | provider settings migration | Proposal + targeted refresh | 2026-04-10 | High | Proposal now explicitly reseeds `.codexACP` after deleting old `.codex` rows. | Credential continuity can break if UUID/secret mapping is not preserved. | Primary operator migration surface. |
| NAV-02 | settings export/import flow | Baseline + proposal | 2026-04-10 | High | Transfer packages rely on provider-specific secret placeholders derived from UUID. | Cross-machine continuity can break if the row UUID changes without secret remap. | Cross-machine continuity surface. |
| NAV-03 | benchmark/sign-off surfaces | Baseline + targeted refresh | 2026-04-10 | High | MVP boundary/UI/evaluation still freeze the old provider set. | Proposal can land with sign-off and runtime boundary docs/policy stale. | Stable boundary surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ProviderAdapter.swift` | Secrets | provider secret-key derivation | 2026-04-10 | High | Secrets are keyed by `provider.<uuid>`, not by provider family. | Delete-and-reseed migration can orphan valid credentials. | Main continuity blocker. |
| MAP-02 | `Chainworks Forge/Support/SettingsTransferService.swift` | Settings transfer | placeholder export/import continuity | 2026-04-10 | High | Exported placeholders are derived from the provider UUID-bound secret key. | Cross-machine continuity is broken if UUID changes silently. | Main transfer blocker. |
| MAP-03 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Durable settings | default seeding and provider-row identity | 2026-04-10 | High | Seeded providers get fresh UUIDs; proposal says deleted Codex rows are replaced by a fresh seeded `.codexACP`. | Proposal needs an explicit UUID strategy or remediation contract. | Main migration owner. |
| MAP-04 | `Chainworks Forge/Support/MVPBoundaryPolicy.swift` | Boundary policy | canonical MVP provider set in code | 2026-04-10 | High | Frozen policy still says `codex`, `claude_code`, `gemini`. | Proposal can change canonical provider identifiers without updating the code owner. | Main boundary blocker. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Provider credential continuity | `ProviderAdapter.secretKey(for:)`, `ProviderSettingsStore`, proposal `3.6a` | local persistence + keychain | 2026-04-10 | High | Old Codex row deletion creates a new provider identity unless explicitly preserved. | Proposal can strand a valid local secret behind an orphaned UUID. | Critical migration gap. |
| DATA-02 | Transfer placeholder continuity | `SettingsTransferService`, proposal `3.6a` | exported/imported machine state | 2026-04-10 | High | Placeholder lists are bound to old provider IDs; reseeded rows use new IDs. | Proposal's cross-machine continuity claim can fail for Codex. | Critical transfer gap. |
| DATA-03 | Canonical provider boundary | `runtime-contract.md`, `mvp-sign-off.md`, `MVPBoundaryPolicy.swift`, proposal `3.6a` | docs + code policy | 2026-04-10 | High | Proposal introduces `codex_acp / claude_acp / gemini_acp` as new canonical identifiers, but boundary owners still freeze the old set. | Implementation can leave the system with conflicting canonical provider vocabularies. | High boundary gap. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | keychain + provider row identity | Current repo + proposal | 2026-04-10 | High | Provider identity and secret identity are coupled through the provider UUID. | Proposal delete-and-reseed path is unsafe without explicit continuity or remediation. | Critical architecture blocker. |
| INT-02 | canonical MVP provider boundary | Stable docs + code policy | 2026-04-10 | High | Runtime boundary, sign-off boundary, and code policy still freeze the old provider identifiers. | Proposal-owned vocabulary migration remains partial. | High architecture blocker. |
| INT-03 | repository gate ownership | Proposal + repo | 2026-04-10 | High | `proposal-033` is still not repo-real, but the proposal now defines its intended shape and transfer-path proof. | This is no longer the main proposal-text blocker. | Verification baseline. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02 | provider migration, transfer proof lane | Major entry surfaces are explicitly owned. |
| Happy path | Partial | DOC-01, DATA-01, DATA-02 | provider migration and transfer continuity | Codex continuity remains partial because UUID/secret remap is unspecified. |
| Historical-read compatibility | Specified | DOC-01 | legacy operator wording and runtimeSessionID rename are now explicit | Previous blocker is closed. |
| Validation error | Specified | DOC-01, DATA-02 | settings transfer import validation | Closed enough except for Codex UUID continuity. |
| Backend error | Specified | DOC-01 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01 | readiness and troubleshooting | Secondary to the main blockers. |
| Retry / recovery | Deferred intentionally | DOC-01 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Rollback / cancellation | Partial | DOC-01, DOC-07 | implementation hold behind `P030` | External hold remains explicit. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-10 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | contract + proof now explicit for most paths | no special rollback beyond hold | 2026-04-10 | High | Main remaining gap is Codex secret continuity. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-10 | High | Proposal text now defines the intended gate shape well enough; remaining blockers are not generic proof-lane omissions. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-10 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and transfer packages | proposal now explicitly names both local and transfer-path proof | add Codex UUID/secret continuity proof once the contract is fixed | 2026-04-10 | High | Follows from the remaining Codex migration gap. |
| TEST-03 | provider-boundary fallout proof | runtime/sign-off/policy consistency | no explicit proposal proof for boundary-owner updates | add proof once fallout classification is explicit | 2026-04-10 | High | Follows from the remaining provider-boundary gap. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Codex migration continuity | delete old Codex rows and seed fresh `.codexACP` | secrets/placeholders are UUID-bound, so fresh row identity can orphan credentials | 2026-04-10 | High | Proposal still leaves a destructive migration path under-specified. |
| REAL-02 | canonical provider boundary | `codex_acp / claude_acp / gemini_acp` become new identifiers | runtime boundary, sign-off, and policy code still freeze `codex / claude_code / gemini` | 2026-04-10 | High | Proposal-owned vocabulary migration remains incomplete. |
| REAL-03 | previous findings | docs/proof/operator wording/`runtimeSessionID` ownership were incomplete | current proposal now includes those sections explicitly | 2026-04-10 | High | Previous stale findings should not be reused. |
| REAL-04 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-10 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | Runtime-only zero-Goose scope is now internally coherent. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Codex continuity remains partial because UUID/secret behavior is not fixed. |
| Data / API contract | Partial | DATA-01, DATA-02, REAL-01 | Codex row deletion still lacks a keychain/placeholder migration contract. |
| Persistence / caching | Partial | DATA-01, DATA-02, TEST-02 | Credential continuity remains implicit. |
| Permissions / auth expiry | Partial | DATA-01 | Auth continuity for deleted Codex rows is unresolved. |
| Feature flags / rollout / rollback | Specified | FLAG-01, FLAG-02 | External hold is explicit. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is adequate for proposal-readiness. |
| Testing strategy | Partial | TEST-02, TEST-03 | Proof awaits a fixed Codex continuity contract and explicit boundary fallout. |
| Dependencies / integration points | Partial | DATA-03, REAL-02 | Provider-vocabulary fallout remains partial across canonical owners. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current runtime/provider baseline, not only remove transport files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: should Codex migration preserve the old provider UUID specifically to preserve keychain continuity?
- QUESTION-02: are `codex_acp / claude_acp / gemini_acp` intended to replace the canonical MVP provider boundary everywhere, including sign-off and policy code?
- BLOCKER-01: Codex migration still lacks UUID/secret continuity or explicit credential-remediation semantics.
- BLOCKER-02: provider-vocabulary migration still omits some canonical MVP boundary owners.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
