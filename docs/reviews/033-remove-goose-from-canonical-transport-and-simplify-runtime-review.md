# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-binding-truth.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/run-control.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/skill-resolution-and-runtime-integration.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/test-suite-architecture.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/test-gates.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/agent-ui-test-execution.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/README.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for durable provider settings and settings-transfer persistence
  - targeted doc refresh for authoritative Goose-bearing runtime/provider/test references
  - targeted verification refresh for proposal-owned proof lane coverage
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing migration tables, docs table, and `proposal-033` gate are now closed in the proposal text
  - the docs migration still omits some baseline-authoritative Goose-bearing refs outside the current table
  - the proof lane still does not explicitly prove `chainworks-settings.json` / `SettingsTransferService` migration, even though `3.6a` owns that path
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong, but not fully closed`
- What improved:
  1. The proposal now explicitly owns durable migration tables, field-level provider-row migration, docs migration, and a concrete `proposal-033` gate shape.
  2. The goal wording is correctly narrowed to Goose runtime references, excluding brand/design metaphor.
  3. The previous review's headline blockers about missing migration tables, missing docs table, and missing gate block are no longer valid on current `HEAD`.
- What still blocks `Green`:
  1. The docs/reference migration still leaves some authoritative Goose-bearing runtime/provider/test docs unclassified.
  2. The proof lane does not explicitly cover `SettingsTransferService` import migration for `chainworks-settings.json`, despite that path being in scope in `3.6a`.

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
| iOS Architecture | Amber | High | Complete | 0 | 1 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-03`, `DOC-04`, `DOC-06`, `DOC-07`, `DOC-08`, `DOC-09`, `DOC-10`, `MAP-03`, `REAL-01`
  Why it matters: The docs layer is materially better than in the previous pass, but the current table in `3.9` still stops short of the full baseline-authoritative Goose-bearing reference set. The stable baseline currently routes runtime/provider/test truth not only through the docs already listed in the proposal, but also through `provider-binding-truth.md`, `run-control.md`, `skill-resolution-and-runtime-integration.md`, and `agent-ui-test-execution.md`. Those docs still carry Goose-shaped language or links today. Without explicitly classifying them as `rewrite`, `delete`, or `transitively superseded`, implementation can finish with part of the canonical reference layer silently stale even after the main transport/provider docs are cleaned up.
  Recommended fix: expand `3.9` from "large obvious docs" to "all baseline-authoritative Goose-bearing runtime/provider/test docs." At minimum, add explicit end states for `provider-binding-truth.md`, `run-control.md`, `skill-resolution-and-runtime-integration.md`, and `agent-ui-test-execution.md`.
  Acceptance criteria:
  - every baseline-authoritative Goose-bearing runtime/provider/test doc is explicitly classified
  - `docs/reference/current-system-baseline.md` and `docs/reference/README.md` cannot still route readers into stale Goose-era truth after `P033`
  - transitive updates are named explicitly where the proposal does not rewrite a doc directly
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `DOC-05`, `MAP-01`, `MAP-02`, `MAP-04`, `TEST-02`, `REAL-02`
  Why it matters: `3.6a` now correctly owns both `provider-settings.json` and `chainworks-settings.json` migration semantics, including `SettingsTransferService` import behavior. But the proof lane in section `2` still only requires `Proposal033Tests` to prove `ProviderSettingsStore.migrateFromGooseEra()`. That leaves the cross-machine import path under-specified in the proposal's own verification contract. Current repo truth treats `chainworks-settings.json` as a durable operator surface, so proposal-owned proof should cover imported settings packages too, not only the local store file.
  Recommended fix: extend the `Proposal033Tests` proof list or `proposal-033` acceptance gate so it explicitly proves `SettingsTransferService` import migration for a Goose-era `chainworks-settings.json` package, including deleted Codex Goose rows and migrated Claude/Gemini rows.
  Acceptance criteria:
  - the proof lane explicitly covers `SettingsTransferService` import migration, not only `ProviderSettingsStore`
  - a Goose-era exported settings package can be imported without deserialization failure or stale Goose row leakage
  - proof expectations match the migration scope already claimed in `3.6a`
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal now owns a real docs table, but the current baseline-authoritative reference set is still wider than that table.
  Tradeoff: keeping the docs matrix compact improves readability, but it weakens closure for a slice that explicitly rewrites the canonical runtime baseline.
  Decision: expand the docs matrix to cover the full authoritative runtime/provider/test Goose-bearing set, or explicitly mark transitive supersession.
  Owner: proposal author

- Conflict: the migration contract now covers `SettingsTransferService`, but the proof lane still checks only local-store migration.
  Tradeoff: keeping the proof list short simplifies the gate, but it leaves a durable operator path outside explicit proposal proof.
  Decision: add one proof assertion for imported settings-package migration.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Expand `3.9` to classify all baseline-authoritative Goose-bearing runtime/provider/test docs | iOS Architecture | Proposal author | Before implementation | current baseline + reference index | no baseline doc remains silently stale after `P033` | `ARCH-033-001` |
| P2 | Extend the proof lane to include `SettingsTransferService` import migration for `chainworks-settings.json` | iOS Architecture | Proposal author | Before implementation | current settings transfer contract | proof lane covers all migration surfaces claimed by `3.6a` | `ARCH-033-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Reference migration | authoritative runtime/provider/test refs become self-consistent | expanded docs matrix with explicit classification | no stable reference layer silently points at Goose-owned truth | next rereview of `P033` | hold if baseline-authoritative Goose-bearing docs remain unowned |
| Durable settings proof | both local and imported settings migration paths remain valid | proof lane includes store migration + transfer import migration | no Goose-era settings package breaks import or leaks stale provider rows | next rereview of `P033` | hold if `chainworks-settings.json` migration remains outside explicit proof |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should the omitted baseline-authoritative docs be rewritten directly in `P033`, or explicitly marked as transitively superseded by a smaller owned doc set?
- QUESTION-02: should the `SettingsTransferService` migration proof live inside `Proposal033Tests`, `ProviderPlatformTests`, or both?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
