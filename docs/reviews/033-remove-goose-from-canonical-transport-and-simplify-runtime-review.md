# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md`
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
  - targeted code refresh for persistent session/model fields and support-bundle fallout
  - targeted doc refresh for canonical subsystem docs that still encode Goose model/executor truth
  - targeted verification refresh for proposal-owned proof lane coverage
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Current repo tensions found:
  - the previous stale findings about missing migration tables, docs table, and `SettingsTransferService` proof are now closed in the proposal text
  - the proposal now contains an internal contradiction between "zero Goose in operator-facing strings" and the historical legacy strings it still prescribes
  - the model/persistence layer is still under-owned: `AgentExecution.gooseSessionID` and related support-bundle/domain-model fallout are not classified anywhere in the proposal
  - `P030` remains red, so implementation is still operationally blocked behind the proposal's own prerequisite gate

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Substantially improved, but internally inconsistent`
- What improved:
  1. The proposal now explicitly owns durable migration tables, `SettingsTransferService` proof, expanded docs migration, and a concrete `proposal-033` gate shape.
  2. The earlier findings about missing docs/proof sections are now stale and should not be reused.
- What still blocks `Green`:
  1. The proposal cannot simultaneously require zero Goose operator strings and also prescribe Goose-labeled historical trust and blocked-run messages.
  2. The proposal still does not own the persisted `gooseSessionID` model field or its doc/export fallout, despite claiming zero Goose runtime references in Swift source.

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
| UI | Red | High | Complete | 1 | 0 | 0 | 0 |
| UX | Red | High | Complete | 1 | 0 | 0 | 0 |
| iOS Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 Cross-Discipline Findings
- Finding ID: `CORE-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `REAL-01`
  Why it matters: The proposal now contradicts itself on operator-facing wording. Section `3.9` defines `"Zero Goose runtime references"` as zero in operator-facing UI strings, and acceptance criterion `11` says UI surfaces have zero `"Goose"` in operator-facing strings. But section `4` still tells the operator `"This run used the Goose runtime which has been removed..."`, and section `6` still defines display labels `"Legacy (unverified) — historical Goose runs"` and `"Legacy (verified) — historical Goose runs"`. Implementation cannot satisfy both the scope/acceptance rule and the prescribed operator copy as written.
  Recommended fix: choose one contract and lock it. Either:
  1. strict zero-Goose operator wording, with historical surfaces rewritten to neutral legacy wording such as `Legacy runtime`, or
  2. explicit carve-out for historical blocked-run/trust surfaces, with acceptance updated to exclude those legacy explanations from the zero-string rule.
  Acceptance criteria:
  - the goal, scope clarification, historical-run handling, trust model, and acceptance list all describe the same operator-facing wording policy
  - there is no proposal-internal requirement to both remove and preserve Goose in user-visible copy
  Confidence: `High`

### 5.2 iOS Architecture Findings
- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-02`, `DOC-03`, `MAP-01`, `MAP-02`, `REAL-02`
  Why it matters: The proposal still does not own the persistent model/storage fallout for `AgentExecution.gooseSessionID`. Current code keeps `gooseSessionID` as the durable SwiftData field and exposes `runtimeSessionID` only as a compatibility accessor in [AgentExecution.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Models/AgentExecution.swift#L14). Support export still serializes the key as `gooseSessionID` in [SupportBundleExporter.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Engine/SupportBundleExporter.swift#L145), and the canonical model doc still documents it that way in [domain-model.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/domain-model.md#L132). At the same time, the proposal goal claims zero Goose runtime references in Swift source. Without an explicit model-layer decision, implementers have to invent whether `gooseSessionID` is renamed with a real data migration, preserved as a grandfathered storage alias, or intentionally excluded from the zero-Goose rule.
  Recommended fix: add a model/storage sub-section that fixes the fate of `AgentExecution.gooseSessionID` and its fallout. At minimum, specify:
  1. whether the persisted field is renamed or retained as a compatibility column,
  2. whether `SupportBundleExporter` keeps or rewrites the serialized key,
  3. and that `domain-model.md` plus [execution-truth-and-recovery.md](/Users/user/Documents/Chainworks%20Forge/docs/reference/execution-truth-and-recovery.md#L1) are updated to match the chosen model/executor vocabulary.
  Acceptance criteria:
  - the proposal explicitly classifies `AgentExecution.gooseSessionID`
  - the zero-Goose-in-Swift-source goal is reconciled with the persisted model strategy
  - support-bundle and stable-doc fallout are named, not left to implementation guesswork
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants zero Goose operator strings while also preserving explicit Goose wording for historical runs.
  Tradeoff: explicit historical wording is clearer to operators, but it violates the proposal's own zero-Goose operator-string contract.
  Decision: the proposal must pick one user-facing language policy and make scope + acceptance consistent with it.
  Owner: proposal author

- Conflict: the proposal aims for zero Goose runtime references in Swift source, but the persisted `AgentExecution` model still uses `gooseSessionID` as durable storage.
  Tradeoff: keeping the field avoids a data migration, but it weakens the stated simplification goal unless explicitly grandfathered.
  Decision: the proposal must either own a real model migration or explicitly grandfather the storage alias and narrow the goal.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Reconcile the zero-Goose operator-string rule with historical blocked-run and trust copy | Cross-discipline | Proposal author | Before implementation | current sections `3.9`, `4`, `6`, `7` | no proposal-internal contradiction remains in operator-facing wording | `CORE-033-001` |
| P1 | Add explicit model/storage contract for `AgentExecution.gooseSessionID` and related doc/export fallout | iOS Architecture | Proposal author | Before implementation | current persistent model + support bundle + stable docs | implementer does not have to invent schema compatibility behavior | `ARCH-033-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Operator wording | goal/scope/acceptance and historical run messaging become internally consistent | one chosen legacy wording policy | no contradictory operator-copy requirements inside the proposal | next rereview of `P033` | hold if proposal still both bans and prescribes Goose-labeled UI strings |
| Persistent model strategy | `gooseSessionID` fate becomes explicit across code/docs/export surfaces | model-layer subsection and proof expectations | no hidden schema decision left to implementation | next rereview of `P033` | hold if model/storage compatibility remains implicit |
| External dependency | `P030` readiness | `P030` audit turns green | `P033` implementation does not start early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The remaining issues are proposal-text closure issues, not missing local evidence.

### Open Questions
- QUESTION-01: should historical blocked-run/trust surfaces say `Goose` explicitly, or should they move to neutral `Legacy runtime` wording?
- QUESTION-02: is `gooseSessionID` intended to remain as a grandfathered storage alias, or should `P033` own a real persisted-model migration?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
