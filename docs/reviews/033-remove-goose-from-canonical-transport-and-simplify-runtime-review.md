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
  - the previous stale findings about missing docs coverage, missing `SettingsTransferService` proof, operator-facing Goose wording, `gooseSessionID` ownership, Codex UUID continuity, provider-boundary fallout, transfer-proof wording, and gate alias ambiguity are now closed in the proposal text
  - durable settings migration still needs an explicit pre-decode owner because current local and transfer paths decode typed enums before any migration hook can run
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved and close to handoff, but one core persistence seam remains under-specified`
- What improved:
  1. The proposal now explicitly owns `SettingsTransferService` proof, neutral legacy operator wording, persistent-model renaming for `runtimeSessionID`, Codex re-auth semantics, the missing provider-boundary fallout (`runtime-contract`, `mvp-sign-off`, `MVPBoundaryPolicy.swift`), and the historical `proposal-029` gate alias.
  2. The earlier findings about docs-table gaps, proof-lane gaps, operator-string contradiction, missing `gooseSessionID` ownership, missing Codex continuity semantics, missing provider-boundary owners, transfer-proof wording, and gate-name ambiguity are now stale and should not be reused.
- What still blocks `Green`:
  1. The proposal still does not lock the raw pre-decode migration owner needed to make old Goose-era JSON readable after enum-case deletion.

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
  Why it matters: `3.6a` correctly recognizes that deleting enum cases breaks deserialization, and it says `ProviderSettingsStore.migrateFromGooseEra()` should run “once on load” while `SettingsTransferService` applies the same migration on imported JSON before merging ([033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L241)). But current repo owners still decode typed models first: [ProviderSettingsStore.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ProviderSettingsStore.swift#L81) decodes `ProviderSettings` directly, and [SettingsTransferService.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/SettingsTransferService.swift#L82) decodes `ExportableSettingsPackage` directly. Once `.codex`, `.claude`, `.gemini`, and `.gooseServer` cases are removed, old Goose-era JSON cannot reach a post-decode migration hook. The proposal still leaves the crucial pre-decode owner implicit.
  Recommended fix: lock one explicit raw-data migration seam for both local and transfer paths. For example:
  1. define a raw JSON migration helper or wire schema that rewrites old family/transport values before any `JSONDecoder().decode(...)` into `ProviderSettings` / `ExportableSettingsPackage`, and
  2. explicitly state whether `migration_version` lives inside `ProviderSettings`, in a wrapper payload, or only in the raw migration layer, plus how `secretPlaceholders` are rewritten before placeholder validation.
  Acceptance criteria:
  - the proposal explicitly says migration happens on raw persisted/imported JSON before typed enum decoding
  - the same owner or helper is named for both `provider-settings.json` and `chainworks-settings.json`
  - `migration_version` and `secretPlaceholders` ownership are explicit enough that an implementer does not need to invent schema behavior
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal defines the row-by-row migration semantics, but current persistence owners decode typed enums before any migration point.
  Tradeoff: keeping the proposal at the semantic level reads cleanly, but implementers can still choose a post-decode hook that can never see old Goose-era payloads once enum cases are removed.
  Decision: the proposal must explicitly own a raw pre-decode migration seam.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add an explicit raw pre-decode migration owner for local and transfer settings payloads | iOS Architecture | Proposal author | Before implementation | current typed decode boundaries | old Goose-era JSON can still be migrated after enum-case removal without invented schema behavior | `ARCH-033-001` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Durable settings migration | old Goose-era local and transfer payloads remain readable after enum-case removal | raw migration owner is explicit for both load and import paths | no implementation is forced to invent a pre-decode seam | next rereview of `P033` | hold if migration can still be interpreted as post-decode only |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should the proposal name a dedicated raw migration helper/wire payload, or is it enough to define the pre-decode seam abstractly?
- QUESTION-02: where exactly should `migration_version` live so local settings and transfer packages share one durable migration contract?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
