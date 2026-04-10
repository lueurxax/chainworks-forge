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
  - the previous stale findings about missing docs coverage, missing `SettingsTransferService` proof, operator-facing Goose wording, `gooseSessionID` ownership, Codex UUID continuity, provider-boundary fallout, transfer-proof wording, gate alias ambiguity, and missing pre-decode seam are now closed in the proposal text
  - durable settings migration now names a raw pre-decode helper, but it still does not fully lock how that helper handles the two different persisted payload shapes: bare `ProviderSettings` versus wrapped `ExportableSettingsPackage`
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved and close to handoff, but one core persistence/schema seam remains under-specified`
- What improved:
  1. The proposal now explicitly owns `SettingsTransferService` proof, neutral legacy operator wording, persistent-model renaming for `runtimeSessionID`, Codex re-auth semantics, the missing provider-boundary fallout (`runtime-contract`, `mvp-sign-off`, `MVPBoundaryPolicy.swift`), the historical `proposal-029` gate alias, and a concrete raw pre-decode migration seam.
  2. The earlier findings about docs-table gaps, proof-lane gaps, operator-string contradiction, missing `gooseSessionID` ownership, missing Codex continuity semantics, missing provider-boundary owners, transfer-proof wording, gate-name ambiguity, and missing pre-decode migration ownership are now stale and should not be reused.
- What still blocks `Green`:
  1. The proposal still does not lock the schema-aware raw migration contract for both persisted payload shapes, so the transfer package path remains partly implicit.

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
| iOS Architecture | Amber | High | Complete | 0 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `MAP-01`, `MAP-02`, `DATA-01`, `REAL-01`
  Why it matters: `3.6a` is materially better now: it explicitly requires raw pre-decode migration and even sketches `migrateRawJSONIfNeeded(_:)` plus the two call sites ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L241)). But the proposal still treats local settings and transfer bundles as if they shared one flat raw shape. The sample/helper comments operate on top-level `configuredProviders` / `preferredProviderIDsByFamily`, which matches `ProviderSettings`, while the real transfer import path decodes a wrapped [ExportableSettingsPackage](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/SettingsTransferService.swift#L3) where `providerSettings` is nested and `secretPlaceholders` live at the top level. That means the proposal still leaves an implementation-critical question open: is there one wrapper-aware raw migrator, two schema-specific migrators, or an intermediate wire type? Without that choice, the transfer path still requires implementer invention.
  Recommended fix: keep the new raw pre-decode requirement, but make it schema-aware. Explicitly choose one of:
  1. a single wrapper-aware raw migrator that branches on payload shape (`ProviderSettings` vs `ExportableSettingsPackage`), or
  2. two named raw migrators, one for local settings payloads and one for transfer packages.
  The proposal should also state exactly where `secretPlaceholders` are rewritten/dropped for deleted Codex rows in the transfer-package path.
  Acceptance criteria:
  - the proposal explicitly distinguishes the raw shape of `provider-settings.json` from `chainworks-settings.json`
  - the proposal names either one wrapper-aware raw migrator or two schema-specific raw migrators
  - `secretPlaceholders` rewrite/drop semantics are explicit on the wrapped transfer-package path before placeholder validation runs
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal defines the row-by-row migration semantics, but current persistence owners decode typed enums before any migration point.
  Tradeoff: the new raw pre-decode seam keeps the proposal much safer, but it still compresses two different persisted schemas into one generic helper shape.
  Decision: the proposal must explicitly own a schema-aware raw migration contract for both local settings and transfer packages.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Make the raw pre-decode migration contract schema-aware for both local settings and transfer packages | iOS Architecture | Proposal author | Before implementation | current typed decode boundaries and transfer wrapper schema | old Goose-era local and transfer payloads can be migrated without inventing wrapper logic during implementation | `ARCH-033-001` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Durable settings migration | old Goose-era local and transfer payloads remain readable after enum-case removal | raw migration contract is explicit for both payload shapes | no implementation is forced to invent wrapper-aware migration logic | next rereview of `P033` | hold if migration still compresses both schemas into an implicit generic helper |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should the proposal name a dedicated raw migration helper/wire payload, or is it enough to define the pre-decode seam abstractly?
- QUESTION-02: should transfer-package migration be handled by the same raw helper with shape detection, or by a separate `migrateRawTransferPackageIfNeeded(_:)` contract?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
