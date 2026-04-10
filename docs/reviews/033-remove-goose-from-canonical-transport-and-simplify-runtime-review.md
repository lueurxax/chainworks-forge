# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/mvp-sign-off.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/README.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for provider UUID/secret storage coupling
  - targeted code refresh for frozen MVP provider boundary ownership
  - targeted doc refresh for runtime/sign-off boundary docs
  - targeted persistence refresh for typed decode boundaries in provider settings import/load
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing docs coverage, missing `SettingsTransferService` proof, operator-facing Goose wording, `gooseSessionID` ownership, Codex UUID continuity, provider-boundary fallout, transfer-proof wording, gate alias ambiguity, missing pre-decode seam, and missing schema-aware payload split are now closed in the proposal text
  - durable settings migration now locks two schema-specific raw migrators, but the section still leaves the final API/call-site contract internally inconsistent (`migrateRawProviderSettings` / `migrateRawTransferPackage` vs `migrateRawJSONIfNeeded(_)`)
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved and nearly implementation-ready, but one migration-API detail remains inconsistent`
- What improved:
  1. The proposal now explicitly owns `SettingsTransferService` proof, neutral legacy operator wording, persistent-model renaming for `runtimeSessionID`, Codex re-auth semantics, the missing provider-boundary fallout (`runtime-contract`, `mvp-sign-off`, `MVPBoundaryPolicy.swift`), the historical `proposal-029` gate alias, a concrete raw pre-decode migration seam, and the two different payload shapes for local versus transfer settings.
  2. The earlier findings about docs-table gaps, proof-lane gaps, operator-string contradiction, missing `gooseSessionID` ownership, missing Codex continuity semantics, missing provider-boundary owners, transfer-proof wording, gate-name ambiguity, missing pre-decode migration ownership, and missing schema-aware payload handling are now stale and should not be reused.
- What still blocks `Green`:
  1. The proposal still does not lock one consistent migration API surface at the call-site level after introducing the two raw migrators.

## 2. Proposal Scope and Completeness
- In scope:
  - complete Goose runtime removal
  - ACP-only transport / session / executor / provider runtime architecture
  - durable settings migration for provider/platform state
  - historical Goose-run blocking and trust fallback
  - stable-reference migration and proof-gate ownership
- Out of scope:
  - completing `P030`
  - converting old Goose runs into ACP runs
  - runtime-heavy proof during proposal review
- External hold:
  - `P030` is still `Not Implemented / Not Ready`, so implementation cannot start yet; this is an operational hold, not the main proposal-text blocker for this pass

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 0 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-01`, `MAP-02`, `DATA-01`, `REAL-01`
  Why it matters: `3.6a` now does the hard part correctly: it distinguishes the two payload shapes and introduces `migrateRawProviderSettings(_:)` plus `migrateRawTransferPackage(_:)` with explicit `secretPlaceholders` handling ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L243)). But the same section still says both call sites invoke `migrateRawJSONIfNeeded(_:)` ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L303)), which is no longer the API surface it just defined. That leaves one last handoff ambiguity: should there be an umbrella dispatcher helper, or should each call site invoke its schema-specific migrator directly?
  Recommended fix: make the call-site contract consistent with the API you want. Either:
  1. keep the two schema-specific helpers and say `ProviderSettingsStore` calls `migrateRawProviderSettings(_:)` while `SettingsTransferService` calls `migrateRawTransferPackage(_:)`, or
  2. introduce an explicit wrapper `migrateRawJSONIfNeeded(_:, shape:)` / dispatcher and show both call sites using it.
  Acceptance criteria:
  - the proposal uses one consistent helper/API naming scheme throughout `3.6a`
  - the local and transfer call sites are each tied to a concrete raw migrator contract
  - rereviewers do not have to infer whether `migrateRawJSONIfNeeded(_)` is stale text or an intended dispatcher
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal defines the row-by-row migration semantics, but current persistence owners decode typed enums before any migration point.
  Tradeoff: the new raw pre-decode seam and schema split solve the real persistence risk, but the section still mixes two API shapes.
  Decision: the proposal must choose one migration helper contract and keep the call-site text aligned with it.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Align the `3.6a` call-site text with the now schema-specific raw migrator API | iOS Architecture | Proposal author | Before implementation | current typed decode boundaries and transfer wrapper schema | implementation handoff no longer has to guess between dispatcher and direct migrator calls | `ARCH-033-001` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Durable settings migration | old Goose-era local and transfer payloads remain readable after enum-case removal | raw migration contract and call-site API names converge | no implementation handoff ambiguity remains around which helper each path should call | next rereview of `P033` | hold if `3.6a` still mixes dispatcher and schema-specific helper names |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should the final API keep the two explicit helpers, or introduce a small dispatcher wrapper over them?
- QUESTION-02: if a dispatcher exists, should the proposal name it explicitly or just show the direct call-site mapping instead?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
