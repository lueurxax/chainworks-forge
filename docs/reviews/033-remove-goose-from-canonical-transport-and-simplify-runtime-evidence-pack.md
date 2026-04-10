# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md` | 2026-04-10 | High | Current `HEAD` proposal now includes transfer-path proof, neutral historical operator wording, explicit `runtimeSessionID` migration, Codex re-auth semantics, canonical boundary fallout, and a fully aligned schema-specific raw migration contract. | Review could keep stale blockers from the previous pass. | Primary proposal source. |
| DOC-02 | `docs/reference/runtime-contract.md` | 2026-04-10 | High | Stable runtime boundary still freezes MVP providers as `codex`, `claude_code`, `gemini`. | Needed to verify that the proposal explicitly classifies this boundary owner. | Runtime boundary reference. |
| DOC-03 | `docs/reference/mvp-sign-off.md` | 2026-04-10 | High | Stable MVP sign-off still defines the frozen provider set as `codex`, `claude_code`, `gemini`. | Needed to verify that the proposal explicitly classifies this boundary owner. | Sign-off boundary reference. |
| DOC-04 | `.review-baselines/current-system-baseline.md` | 2026-04-10 | High | Review intake still says current runtime is Goose-backed and current MVP provider families are `codex`, `claude_code`, `gemini`. | Proposal must own the boundary rewrite explicitly, not only subset docs. | Reusable baseline intake. |
| DOC-05 | `docs/reference/current-system-baseline.md` | 2026-04-10 | High | Current stable baseline still routes subsystem truth through Goose-era provider/runtime terminology. | Proposal cleanup must stay consistent with the broader reference stack. | Current host-system truth. |
| DOC-06 | `docs/reference/domain-model.md` | 2026-04-10 | High | Canonical model doc still documents the old `gooseSessionID` field today, but the proposal now owns a concrete rename. | Previous persistent-model ownership finding is stale. | Closed model-doc area. |
| DOC-07 | `docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md` | 2026-04-10 | High | `P030` is still `Not Implemented / Not Ready`. | `P033` could be judged as executable today when its prerequisite is still blocked. | External dependency status. |
| DOC-08 | `scripts/test-gate.sh` | 2026-04-10 | High | The repo-owned prerequisite lane still exists as `proposal-029`, and the proposal explicitly explains that historical alias. | Previous gate-alias blocker is stale. | Proof-lane source of truth. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review intake and reusable assumptions | 2026-04-10 | High | Still fresh as entry point; affected migration/boundary surfaces needed targeted refresh. | Baseline entry point. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem map and provider/runtime boundary | 2026-04-10 | High | Fresh overall; provider/runtime boundary still uses the old provider set. | Current system map. |
| BASE-03 | stable provider/runtime/sign-off docs and policy code | Partially refreshed | runtime boundary, sign-off boundary, provider UUID/secret coupling | 2026-04-10 | High | Stable docs plus targeted code inspection were enough to confirm the remaining status is an external hold, not a proposal-text blocker. | Narrow baseline refresh. |
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
  - none found on current `HEAD`
- External hold:
  - `P030` remains red, so implementation still cannot start

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | provider settings migration | Proposal + targeted refresh | 2026-04-10 | High | Proposal explicitly chooses Codex delete-and-reseed with operator re-auth, while Claude/Gemini preserve UUID. | Main residual risk is implementation drift away from the explicit migration contract, not proposal ambiguity. | Primary operator migration surface. |
| NAV-02 | settings export/import flow | Baseline + proposal | 2026-04-10 | High | Transfer packages rely on provider-specific secret placeholders derived from UUID; the proposal drops deleted Codex placeholders intentionally and maps the import path to `migrateRawTransferPackage(_:)`. | Remaining risk is execution quality, not proposal ownership. | Cross-machine continuity surface. |
| NAV-03 | benchmark/sign-off surfaces | Baseline + targeted refresh | 2026-04-10 | High | MVP boundary/UI/evaluation still freeze the old provider set today, but the proposal classifies these owners explicitly. | Previous boundary-owner blocker is closed. | Stable boundary surface. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Providers/ProviderAdapter.swift` | Secrets | provider secret-key derivation | 2026-04-10 | High | Secrets are keyed by `provider.<uuid>`, not by provider family. | Needed to verify the row-level migration semantics against current storage reality. | Migration grounding. |
| MAP-02 | `Chainworks Forge/Support/SettingsTransferService.swift` | Settings transfer | typed decode and placeholder validation | 2026-04-10 | High | `importSettings` decodes `ExportableSettingsPackage` before validation or replacement. | Proposal covers wrapped transfer migration semantically and ties it to a concrete raw migrator call site. | Main transfer-path mapping. |
| MAP-03 | `Chainworks Forge/Providers/ProviderSettingsStore.swift` | Durable settings | typed local settings load | 2026-04-10 | High | `load(from:)` decodes `ProviderSettings` directly before any migration hook is visible in current code. | Proposal covers the local raw seam and ties it to a concrete raw migrator call site. | Main local-path mapping. |
| MAP-04 | `Chainworks Forge/Support/MVPBoundaryPolicy.swift` | Boundary policy | canonical MVP provider set in code | 2026-04-10 | High | Frozen policy still says `codex`, `claude_code`, `gemini`. | Needed to verify the proposal explicitly classifies this owner. | Closed boundary area. |
| MAP-05 | `scripts/test-gate.sh` | Proof lane | current repo-owned prerequisite gate naming | 2026-04-10 | High | The repo still exposes `proposal-029` / `PROPOSAL_029_TESTS` for the second-wave ACP gate, and the proposal documents that alias explicitly. | Previous gate-name blocker is closed. | Closed gate area. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Provider credential continuity | `ProviderAdapter.secretKey(for:)`, `ProviderSettingsStore`, proposal `3.6a` | local persistence + keychain | 2026-04-10 | High | Proposal explicitly chooses UUID preservation for migrated rows and explicit re-auth for deleted Codex rows. | Residual risk is implementation correctness, not unresolved proposal semantics. | Migration grounding. |
| DATA-02 | Transfer placeholder continuity | `SettingsTransferService`, proposal `3.6a` | exported/imported machine state | 2026-04-10 | High | Placeholder lists are bound to UUID and live on the typed transfer package outside `providerSettings`. | Proposal states the rewrite/drop semantics and names the concrete wrapped-transfer migrator. | Transfer grounding. |
| DATA-03 | Canonical provider boundary | `runtime-contract.md`, `mvp-sign-off.md`, `MVPBoundaryPolicy.swift`, proposal `3.9` | docs + code policy | 2026-04-10 | High | Proposal explicitly classifies the previously missing canonical boundary owners. | Previous boundary-owner blocker is closed. | Closed boundary gap. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | keychain + provider row identity | Current repo + proposal | 2026-04-10 | High | Provider identity and secret identity are coupled through the provider UUID, and the proposal deliberately splits migrated-row continuity from deleted-row re-auth. | Those semantics are owned; remaining risk is implementation fidelity. | Main architecture seam. |
| INT-02 | canonical MVP provider boundary | Stable docs + code policy | 2026-04-10 | High | Runtime boundary, sign-off boundary, and code policy still freeze the old provider identifiers today, but the proposal explicitly owns their rewrite. | Previous architecture blocker is closed. | Closed boundary seam. |
| INT-03 | repository gate ownership | Proposal + repo | 2026-04-10 | High | The repo still names the second-wave ACP lane `proposal-029` while the proposal depends on `P030`, and the proposal documents that alias explicitly. | Previous gate-contract blocker is closed. | Closed gate seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01, NAV-02 | provider migration, transfer proof lane | Major entry surfaces are explicitly owned. |
| Happy path | Specified | DOC-01, DATA-01, DATA-02 | provider migration and transfer continuity | Migration semantics and call-site ownership are explicit. |
| Historical-read compatibility | Specified | DOC-01 | legacy operator wording and runtimeSessionID rename are explicit | Previous blocker is closed. |
| Validation error | Specified | DOC-01, DATA-02, MAP-02 | settings transfer import validation | Wrapped transfer migration semantics and helper ownership are explicit. |
| Backend error | Specified | DOC-01 | ACP-only runtime errors | Core direction is explicit. |
| Offline / degraded | Partial | DOC-01 | readiness and troubleshooting | Secondary to the main review outcome. |
| Retry / recovery | Deferred intentionally | DOC-01 | historical Goose runs blocked rather than converted | Acceptable and explicit. |
| Rollback / cancellation | Partial | DOC-01, DOC-07 | implementation hold behind `P030` | External hold remains explicit. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | hard `P030` prerequisite gate | proposal-level implementation hold | do not start `P033` until `P030` is green | hold implementation entirely | 2026-04-10 | High | Correct and explicit. |
| FLAG-02 | durable provider settings migration | provider-platform continuity | row-level, schema-level, and call-site contracts are explicit | no special rollback beyond hold | 2026-04-10 | High | Proposal-first gap is closed; remaining risk is implementation quality. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `proposal-033` focused proof gate | proposal-readiness verification | post-implementation proof | 2026-04-10 | High | Proposal text defines the intended gate shape well enough; remaining status is the external prerequisite hold. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | prerequisite audit | `P030` readiness | current audit shows `P030` still red | keep `P033` hard-blocked until `P030` is green | 2026-04-10 | High | External hold remains real. |
| TEST-02 | durable settings migration proof | persisted provider settings and transfer packages | proposal explicitly names both local and transfer-path proof | keep tests aligned with the explicit schema-specific helpers and transfer placeholder rewrite | 2026-04-10 | High | Proposal-first gap is closed. |
| TEST-03 | prerequisite gate proof | second-wave ACP dependency lane naming | proposal documents the historical alias explicitly | no further proposal proof gap here | 2026-04-10 | High | Previous gate-contract gap is closed. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | raw settings migration | proposal defines two schema-specific raw migrators and direct call-site mapping | the same section now directly maps `ProviderSettingsStore` to `migrateRawProviderSettings(_:)` and `SettingsTransferService` to `migrateRawTransferPackage(_:)` | 2026-04-10 | High | Previous migration-helper blocker is closed. |
| REAL-02 | previous findings | docs/proof/operator wording/`runtimeSessionID` ownership, Codex continuity semantics, provider-boundary fallout, gate-alias ambiguity, and helper naming were incomplete | current proposal now includes those sections explicitly | 2026-04-10 | High | Previous stale findings should not be reused. |
| REAL-03 | external dependency | implementation starts only after `P030` is green | `P030` is still red | 2026-04-10 | High | Operational start remains blocked. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Simplification goal is clear. |
| Scope boundaries | Specified | DOC-01 | Runtime-only zero-Goose scope is internally coherent. |
| Reusable baseline coverage | Specified | BASE-01, BASE-02, DOC-04, DOC-05 | Local evidence is sufficient. |
| Screen / surface definition | Specified | NAV-01, NAV-02, NAV-03 | Major operator surfaces are mapped. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03 | Entry points are adequately mapped. |
| State handling | Specified | H table | Migration semantics and helper ownership are explicit. |
| Data / API contract | Specified | DATA-01, DATA-02, REAL-01 | Local and transfer migration now have explicit raw-migrator contracts. |
| Persistence / caching | Specified | DATA-01, DATA-02 | Row-by-row persistence semantics are explicit. |
| Permissions / auth expiry | Specified | DATA-01 | Codex re-auth is explicit. |
| Feature flags / rollout / rollback | Specified | FLAG-01, FLAG-02 | External hold is explicit. |
| Analytics / instrumentation | Specified | METRIC-01 | Gate ownership is adequate for proposal-readiness. |
| Testing strategy | Specified | TEST-02 | Proof obligations map cleanly to the explicit helper/call-site migration API. |
| Dependencies / integration points | Specified | DOC-07, DOC-08 | External dependency and gate alias are explicit. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P033` is intended to fully rewrite the current runtime/provider baseline, not only remove transport files.
- ASSUMP-02: historical Goose runs remain readable but non-resumable.
- QUESTION-01: No blocking proposal-first open question remains on current `HEAD`.
- EXTERNAL-HOLD-01: `P030` remains red, so implementation cannot begin.

## O. Research Triggers / External Questions
No external research triggers were required for this proposal-readiness pass. Local proposal/docs/code/baseline evidence was sufficient.
