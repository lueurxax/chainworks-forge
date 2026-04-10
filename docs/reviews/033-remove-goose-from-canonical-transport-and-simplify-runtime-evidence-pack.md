# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-10 | High | Current `HEAD` proposal now includes transfer-path proof, neutral historical operator wording, explicit `runtimeSessionID` migration, Codex re-auth semantics, and canonical boundary fallout. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/reference/runtime-contract.md` | 2026-04-10 | High | Stable runtime boundary still freezes MVP providers as `codex`, `claude_code`, `gemini`. | Needed to verify that the proposal now explicitly classifies this boundary owner. | Runtime boundary reference. |
| DOC-03 | `docs/reference/mvp-sign-off.md` | 2026-04-10 | High | Stable MVP sign-off still defines the frozen provider set as `codex`, `claude_code`, `gemini`. | Needed to verify that the proposal now explicitly classifies this boundary owner. | Sign-off boundary reference. |
| DOC-04 | `.review-baselines/current-system-baseline.md` | 2026-04-10 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the boundary rewrite explicitly, not only subset docs. | Reusable baseline intake. |
| DOC-05 | `docs/reference/current-system-baseline.md` | 2026-04-10 | High | Current stable baseline still routes subsystem truth through Goose-era provider/runtime terminology. | Proposal cleanup must stay consistent with the broader reference stack. | Current host-system truth. |
| DOC-06 | `docs/reference/domain-model.md` | 2026-04-10 | High | Canonical model doc still documents the old `gooseSessionID` field today, but the proposal now owns a concrete rename. | Previous persistent-model ownership finding is stale. | Closed model-doc area. |
| DOC-07 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-10 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable today when its prerequisite is still blocked. | External dependency status. |
| DOC-08 | `scripts/test-gate.sh` | 2026-04-10 | High | The repo-owned prerequisite lane still exists as `proposal-029`, and the proposal now explicitly explains that historical alias. | Previous gate-alias blocker is stale. | Proof-lane source of truth. |

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
  - durable settings migration still does not lock the raw pre-decode owner for local and transfer payloads after enum-case removal
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | provider settings migration | Proposal + targeted refresh | 2026-04-10 | High | Proposal now explicitly chooses Codex delete-and-reseed with operator re-auth, while Claude/Gemini preserve UUID. | The remaining risk is no longer semantic drift; it is where raw JSON migration actually happens before typed decode. | Primary operator migration surface. |
| NAV-02 | settings export/import flow | Baseline + proposal | 2026-04-10 | High | Transfer packages rely on provider-specific secret placeholders derived from UUID; the proposal now drops deleted Codex placeholders intentionally. | Import path still needs an explicit pre-decode/raw-data owner because typed `ExportableSettingsPackage` decode happens before validation. | Cross-machine continuity surface. |
| NAV-03 | benchmark/sign-off surfaces | Baseline + targeted refresh | 2026-04-10 | High | MVP boundary/UI/evaluation still freeze the old provider set today, but the proposal now classifies these owners explicitly. | Previous boundary-owner blocker is closed. | Stable boundary surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ProviderAdapter.swift` | Secrets | provider secret-key derivation | 2026-04-10 | High | Secrets are keyed by `provider.<uuid>`, not by provider family. | Needed to verify the row-level migration semantics against current storage reality. | Migration grounding. |
| MAP-02 | `Chainworks Forge/Support/SettingsTransferService.swift` | Settings transfer | typed decode and placeholder validation | 2026-04-10 | High | `importSettings` decodes `ExportableSettingsPackage` before validation or replacement. | Proposal needs an explicit raw migration seam before typed decode if old enum raw values are removed. | Main transfer-path blocker. |
| MAP-03 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Durable settings | typed local settings load | 2026-04-10 | High | `load(from:)` decodes `ProviderSettings` directly before any migration hook is visible in current code. | Proposal needs an explicit raw migration seam before typed decode for local settings too. | Main local-path blocker. |
| MAP-04 | `Chainworks Forge/Support/MVPBoundaryPolicy.swift` | Boundary policy | canonical MVP provider set in code | 2026-04-10 | High | Frozen policy still says `codex`, `claude_code`, `gemini`. | Needed to verify the proposal now classifies this owner explicitly. | Closed boundary area. |
| MAP-05 | `scripts/test-gate.sh` | Proof lane | current repo-owned prerequisite gate naming | 2026-04-10 | High | The repo still exposes `proposal-029` / `PROPOSAL_029_TESTS` for the second-wave ACP gate, and the proposal now documents that alias explicitly. | Previous gate-name blocker is closed. | Closed gate area. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Provider credential continuity | `ProviderAdapter.secretKey(for:)`, `ProviderSettingsStore`, proposal `3.6a` | local persistence + keychain | 2026-04-10 | High | Proposal now explicitly chooses UUID preservation for migrated rows and explicit re-auth for deleted Codex rows. | Current code still needs a pre-decode seam to even read old payloads before those row-level rules can apply. | High persistence gap. |
| DATA-02 | Transfer placeholder continuity | `SettingsTransferService`, proposal `3.6a` | exported/imported machine state | 2026-04-10 | High | Placeholder lists are bound to UUID and live on the typed transfer package outside `providerSettings`. | Proposal needs to say where raw-package migration rewrites `secretPlaceholders` before placeholder validation runs. | High transfer-schema gap. |
| DATA-03 | Canonical provider boundary | `runtime-contract.md`, `mvp-sign-off.md`, `MVPBoundaryPolicy.swift`, proposal `3.9` | docs + code policy | 2026-04-10 | High | Proposal now explicitly classifies the previously missing canonical boundary owners. | Previous boundary-owner blocker is closed. | Closed boundary gap. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | keychain + provider row identity | Current repo + proposal | 2026-04-10 | High | Provider identity and secret identity are coupled through the provider UUID, and the proposal now deliberately splits migrated-row continuity from deleted-row re-auth. | Those semantics still need an explicit raw migration entry point before typed decode. | Main architecture seam. |
| INT-02 | canonical MVP provider boundary | Stable docs + code policy | 2026-04-10 | High | Runtime boundary, sign-off boundary, and code policy still freeze the old provider identifiers today, but the proposal now explicitly owns their rewrite. | Previous architecture blocker is closed. | Closed boundary seam. |
| INT-03 | repository gate ownership | Proposal + repo | 2026-04-10 | High | The repo still names the second-wave ACP lane `proposal-029` while the proposal depends on `P030`, and the proposal now documents that alias explicitly. | Previous gate-contract blocker is closed. | Closed gate seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02 | provider migration, transfer proof lane | Major entry surfaces are explicitly owned. |
| Happy path | Partial | DOC-01, DATA-01, DATA-02 | provider migration and transfer continuity | Migration semantics are explicit, but the proposal still does not name the raw pre-decode owner that makes old payloads readable. |
| Historical-read compatibility | Specified | DOC-01 | legacy operator wording and runtimeSessionID rename are now explicit | Previous blocker is closed. |
| Validation error | Partial | DOC-01, DATA-02, MAP-02 | settings transfer import validation | Typed package decode and placeholder validation still need a pre-decode migration seam before old raw values can be handled. |
| Backend error | Specified | DOC-01 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01 | readiness and troubleshooting | Secondary to the main blockers. |
| Retry / recovery | Deferred intentionally | DOC-01 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Rollback / cancellation | Partial | DOC-01, DOC-07 | implementation hold behind `P030` | External hold remains explicit. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-10 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | row-level contract is explicit, raw pre-decode seam is not | no special rollback beyond hold | 2026-04-10 | High | Main remaining gap is migration-owner clarity, not semantics. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-10 | High | Proposal text now defines the intended gate shape well enough; remaining blockers are not generic proof-lane omissions. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-10 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and transfer packages | proposal now explicitly names both local and transfer-path proof | add proof that migration runs on raw JSON/package data before typed decode and placeholder validation | 2026-04-10 | High | Follows from the remaining raw-migration gap. |
| TEST-03 | prerequisite gate proof | second-wave ACP dependency lane naming | proposal now documents the historical alias explicitly | no further proposal proof gap here | 2026-04-10 | High | Previous gate-contract gap is closed. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | raw settings migration | proposal says migration happens “once on load” and on imported JSON before merging | current local and transfer owners both decode typed enums first, so a post-decode hook cannot ever see old Goose-era payloads after enum-case deletion | 2026-04-10 | High | Proposal still under-specifies the core migration seam. |
| REAL-02 | previous findings | docs/proof/operator wording/`runtimeSessionID` ownership, Codex continuity semantics, provider-boundary fallout, and gate-alias ambiguity were incomplete | current proposal now includes those sections explicitly | 2026-04-10 | High | Previous stale findings should not be reused. |
| REAL-03 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-10 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | Runtime-only zero-Goose scope is now internally coherent. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Partial | H table | Migration semantics are explicit, but the pre-decode owner is still not locked. |
| Data / API contract | Partial | DATA-01, DATA-02, REAL-01 | Local and transfer migration still rely on an implicit raw-data seam. |
| Persistence / caching | Specified | DATA-01, DATA-02 | Row-by-row persistence semantics are now explicit. |
| Permissions / auth expiry | Specified | DATA-01 | Codex re-auth is now explicit. |
| Feature flags / rollout / rollback | Specified | FLAG-01, FLAG-02 | External hold is explicit. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is adequate for proposal-readiness. |
| Testing strategy | Partial | TEST-02 | Proof still needs an explicit raw pre-decode migration assertion. |
| Dependencies / integration points | Specified | DOC-07, DOC-08 | External dependency and gate alias are now explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current runtime/provider baseline, not only remove transport files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: should the proposal name a concrete raw migration helper/wire payload, or is an abstract pre-decode seam enough?
- QUESTION-02: where exactly should `migration_version` live so local settings and transfer packages share one durable migration contract?
- BLOCKER-01: raw pre-decode migration ownership is still implicit for both `provider-settings.json` and `chainworks-settings.json`.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
