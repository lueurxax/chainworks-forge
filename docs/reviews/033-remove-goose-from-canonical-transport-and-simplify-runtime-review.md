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
  - `/Users/user/Documents/Chainworks Forge/docs/reference/test-gates.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/README.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/chainworks_forge_design_kit_v1.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline refreshed:
  - targeted code refresh for durable provider settings and settings-transfer persistence
  - targeted code refresh for current provider identifier / transport ownership
  - targeted code refresh for historical run-truth readers
  - targeted verification refresh for repository gate ownership
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing migration tables, docs table, and gate block are now closed in the proposal text
  - durable provider migration is still under-specified beyond enum/raw-value rewrite
  - docs migration is substantially better, but still misses some authoritative Goose-bearing refs
  - `P030` remains red, so implementation is still externally blocked

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved`
- What improved:
  1. The proposal now explicitly owns durable migration tables, docs migration, and a concrete `proposal-033` gate shape.
  2. The goal wording is narrowed correctly to Goose runtime references, not brand/design metaphor.
  3. The previous review’s three headline blockers are no longer valid on current `HEAD`.
- What still blocks `Green`:
  1. The durable provider migration contract still stops at enum/raw-value rewrite and leaves the rest of persisted provider state semantically undefined.
  2. The docs/reference migration still omits several authoritative runtime/provider refs that currently encode Goose-backed truth.

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
  - `P030` is still `Not Implemented / Not Ready`, so implementation cannot start yet even if `P033` proposal quality improves further

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `DOC-05`, `MAP-01`, `MAP-02`, `DATA-01`, `REAL-01`
  Why it matters: Section `3.6a` now correctly introduces raw-value migration tables, but it still does not define the full durable migration outcome for persisted `ConfiguredProvider` records. Current durable settings persist more than `family` and `transport`: [ConfiguredProvider.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ConfiguredProvider.swift#L3) stores `displayName`, `endpoint`, `authMode`, `capabilities`, and `adapterVersion`; [ProviderSettingsStore.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Providers/ProviderSettingsStore.swift#L129) seeds Goose-shaped provider rows with Goose labels, endpoint URLs, and auth expectations; [SettingsTransferService.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Support/SettingsTransferService.swift#L56) exports/imports those settings as durable machine state. The proposal currently says `"goose_server" -> "cli"` and deletes legacy Codex rows, but it does not say what happens to stale Goose endpoints, auth modes, display names, capabilities, or to the operator’s effective Codex availability after the old entry is removed.
  Recommended fix: extend `3.6a` with a semantic field-migration contract for `ConfiguredProvider`, not only raw values. Lock what happens to `displayName`, `endpoint`, `authMode`, `capabilities`, `adapterVersion`, and any deleted Goose Codex preference after migration. Also state whether a deleted Goose Codex row is replaced by a seeded/disabled `codexACP` entry, or whether the operator must explicitly reconfigure Codex after migration.
  Acceptance criteria:
  - `ConfiguredProvider` field-by-field migration is explicit, not inferred
  - migrated CLI-backed providers cannot retain stale Goose endpoint/auth assumptions silently
  - the post-migration operator outcome for removed Goose Codex rows is explicit
  - `SettingsTransferService` import/export semantics match the same field-level migration contract
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-03`, `DOC-04`, `DOC-05`, `DOC-07`, `DOC-08`, `DOC-09`, `MAP-03`, `REAL-02`
  Why it matters: The docs migration is no longer broadly missing, but it is still incomplete against the current authoritative reference stack. `P033` now owns nine doc rows in [033-remove-goose-from-canonical-transport-and-simplify-runtime.md](/Users/user/Documents/Chainworks%20Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md#L268), which closes the previous stale finding. However, current baseline authority still explicitly routes runtime/provider truth through additional Goose-bearing refs such as `goose-provider-remediation.md`, `live-provider-execution-slice.md`, `workflow-execution-engine.md`, `per-agent-mcp-policy-and-runtime-validation.md`, and `test-suite-architecture.md` via [current-system-baseline.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/current-system-baseline.md#L43) and [README.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/README.md#L13). As written, the proposal still allows implementation to finish with part of the authoritative reference layer silently stale.
  Recommended fix: expand the docs matrix from “key docs” to “authoritative Goose-bearing docs for runtime/provider/test truth.” At minimum, explicitly classify `goose-provider-remediation.md`, `live-provider-execution-slice.md`, `workflow-execution-engine.md`, `per-agent-mcp-policy-and-runtime-validation.md`, and `test-suite-architecture.md` as `rewrite`, `delete`, or `transitively updated by owned doc X`.
  Acceptance criteria:
  - every authoritative Goose-bearing runtime/provider/test reference is explicitly classified
  - `goose-provider-remediation.md` has an explicit end state
  - the current-system baseline and reference index cannot point at silently stale Goose-owned truth after `P033`
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal now owns durable raw-value migration, but current persisted provider rows encode Goose semantics in more fields than the tables cover.
  Tradeoff: stopping at enum/key migration keeps the proposal shorter, but it leaves implementers to invent user-visible provider outcomes.
  Decision: the proposal should own semantic provider-row migration too.
  Owner: proposal author

- Conflict: the proposal now owns a real docs table, but current authoritative runtime/provider/test refs still exceed that list.
  Tradeoff: keeping the docs matrix compact improves readability, but it weakens closure for a slice that explicitly rewrites the canonical transport baseline.
  Decision: expand the docs matrix to cover all authoritative Goose-bearing refs or explicitly chain them to an owned rewrite.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Extend `3.6a` from raw-value migration to full `ConfiguredProvider` semantic migration | iOS Architecture | Proposal author | Before implementation | current provider settings model | no implementer has to guess how migrated provider rows should behave | `ARCH-033-001` |
| P1 | Expand the docs/reference migration table to cover all authoritative Goose-bearing runtime/provider/test refs | iOS Architecture | Proposal author | Before implementation | current baseline + reference index | no baseline doc remains silently stale after `P033` | `ARCH-033-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Durable provider migration | migrated provider rows remain semantically valid | field-level migration table and explicit Codex replacement/remediation rule | no stale Goose endpoint/auth/display labels survive silently | next rereview of `P033` | hold if migration still only rewrites raw values |
| Reference migration | authoritative runtime/provider/test refs become self-consistent | expanded docs matrix with explicit classification | no stable reference layer silently points at Goose-owned truth | next rereview of `P033` | hold if authoritative Goose-bearing docs remain unowned |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: after deleting a Goose Codex provider row, what exact replacement or remediation path becomes canonical for operators?
- QUESTION-02: which `ConfiguredProvider` fields are intentionally cleared, rewritten, or preserved during migration?
- QUESTION-03: which authoritative Goose-bearing refs are rewritten directly versus transitively superseded by another owned doc?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
