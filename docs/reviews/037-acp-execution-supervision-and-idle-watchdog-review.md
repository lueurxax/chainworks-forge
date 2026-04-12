# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/test-gates.md`
  - `scripts/test-gate.sh`
  - current `037` review/evidence artifacts as stale-basis comparators
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/execution-truth-and-recovery.md`
- Baseline refreshed:
  - targeted reread of `§10–§13` in `P037`
  - targeted code refresh for the shared ACP progress classifier
  - targeted code refresh for watchdog retry ownership and persistence
  - targeted proof-lane refresh in `test-gates.md` and `scripts/test-gate.sh`
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- Targeted context refresh performed:
  - `ExecutionEventBridge`
  - `RuntimeAgentExecutor`
  - `AgentExecution`
  - `ExecutionTruth`
  - `StageRetryCoordinator`
  - `RecoveryCoordinator`
  - `RunReportBuilder`
  - `RuntimeAgentExecutorTests`
  - `scripts/test-gate.sh`
  - `docs/reference/test-gates.md`
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/ExecutionEventBridge.swift`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/ExecutionTruth.swift`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift`
- Current repo contradictions found:
  - `P037 §10.2` maps `search`, `execute`, and `permission:execute` to strong progress, but the shared classifier and runtime watchdog still treat them as weak discovery/read-loop progress.
  - `P037 §11` is partly stale against current `HEAD`: `supervisionClassification` is already persisted on `AgentExecution`, and the repo-owned `proposal-037` gate already exists.
  - the stable reference chain is still incomplete for this slice: `docs/reference/execution-truth-and-recovery.md` does not yet document `supervisionClassification` or the watchdog-specific read-order the proposal depends on.
- Remaining blockers:
  - taxonomy mismatch on the core weak/strong progress contract
  - stale implementation framing / missing stable-doc promotion ownership

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong, but not implementation-ready yet`
- Top residual implementation risks:
  1. The proposal's canonical progress mapping would materially change watchdog behavior on the current repo because `search` and `execute` are currently weak/discovery signals, not strong progress.
  2. The draft still reads like a greenfield implementation slice in places even though key owners (`supervisionClassification`, `proposal-037` gate) are already landed.
  3. Stable reference promotion is not owned clearly enough: the proposal depends on execution-truth changes that the current stable reference layer still does not describe.
- Top opportunities:
  1. Keep the current shared weak/discovery classifier unless the proposal explicitly justifies and migrates away from it.
  2. Reframe `P037` as a delta/migration proposal over the landed watchdog/retry substrate instead of "add the field / add the gate".
  3. Use the proposal to promote `supervisionClassification` and watchdog-specific reader precedence into `docs/reference/execution-truth-and-recovery.md`.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP-wide stream-based supervision
  - fixed watchdog thresholds
  - one automatic fresh retry
  - supervision truth in receipts/reports/recovery/operator UI
  - proof requirements for the watchdog model
- Out of scope:
  - provider-specific policy matrices
  - process/socket heuristics as canonical truth
  - workflow-YAML changes
  - broader transport redesign outside supervision/recovery behavior
- Deferred intentionally:
  - none blocking
- Most important baseline refreshes performed:
  - current weak/strong/mutating progress classification in code
  - current watchdog retry ownership and persistence
  - current gate ownership for `proposal-037`
  - current stable execution-truth reference coverage
- Most important contradictions with current repo:
  - `P037 §10.2` changes the meaning of `search` / `execute` without acknowledging that the current shared classifier and watchdog tests treat them as weak/discovery signals.
  - `P037 §11` still lists "add `supervisionClassification`" and "add `proposal-037` gate" as future work even though both are already present on `HEAD`.
  - the stable execution-truth reference is not yet updated to the proposal's watchdog-specific truth model.
- Most important missing or partial states:
  - stable-doc promotion for the watchdog-specific truth contract

## 3. Proposal Readiness Verdict
- `Readiness = Amber`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is **not** an Evidence Gap Review. Local proposal/docs/code/baseline evidence is sufficient for a proposal-first verdict.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 1 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding. The draft keeps watchdog history subordinate to the current run-detail / focused-timeline shell spine.

### 5.2 UX Findings
- No live UX `proposal-text` finding. The draft preserves operator-visible retry/recovery semantics inside current shell owners.

### 5.3 iOS Architecture Findings

#### ARCH-001 — `P037` progress taxonomy contradicts the current shared classifier and proof surfaces
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `MAP-01`, `MAP-02`, `TEST-01`
- Proposal refs:
  - `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md:593-611`
- Current repo refs:
  - `Chainworks Forge/Engine/ExecutionEventBridge.swift:430-448`
  - `Chainworks Forge/Engine/RuntimeAgentExecutor.swift:1043-1057`
  - `Chainworks ForgeTests/RuntimeAgentExecutorTests.swift:286-295`
- Why it matters:
  - The proposal's answer to product question `#1` ("What counts as meaningful progress?") is not aligned with current repo truth. It says `search`, `execute`, and `permission:execute` are strong progress, while the shared classifier and current watchdog tests treat them as weak/discovery progress. That changes when the read-loop watchdog fires and would materially alter behavior across ACP families.
- Required fix:
  - Either keep the current weak/discovery mapping in `§10.2`, or explicitly frame the strong-mapping variant as a deliberate migration from current repo reality with updated acceptance language and proof expectations.

#### ARCH-002 — `P037` still mixes landed owners with future work and does not own stable-doc promotion
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-04`, `MAP-03`, `TEST-02`
- Proposal refs:
  - `docs/proposals/037-acp-execution-supervision-and-idle-watchdog.md:648-677`
- Current repo refs:
  - `Chainworks Forge/Models/AgentExecution.swift:49-58`
  - `scripts/test-gate.sh:146-175`
  - `docs/reference/test-gates.md:427-462`
  - `docs/reference/execution-truth-and-recovery.md:69-90`
- Why it matters:
  - The implementation plan still says "add `supervisionClassification`" and "add `proposal-037` gate" even though those owners already exist on `HEAD`. At the same time, the stable execution-truth reference still does not document the watchdog-specific truth contract the proposal now depends on. That combination makes the draft partially stale as an implementation guide and leaves the stable dependency chain underspecified.
- Required fix:
  - Rewrite `§11` as a landed-vs-remaining delta plan, and explicitly own promotion of watchdog-specific truth into the stable execution-truth reference layer.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  - The draft says the shared classifier is the taxonomy owner, but its initial mapping table changes core tool semantics from the current owner implementation without calling that a migration.
  - Decision needed: keep current weak/discovery semantics for `search` / `execute`, or explicitly migrate away from them.
- Conflict:
  - The proposal describes some core watchdog pieces as future work even though they are already landed.
  - Decision needed: convert the document from greenfield implementation framing to current-head delta framing.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Resolve the weak/strong taxonomy for `search` / `execute` / `permission:execute` and align `§10.2` with the chosen repo truth | iOS Architecture | proposal author | Before next review | current classifier + watchdog tests | proposal and shared classifier no longer disagree on core progress semantics | `ARCH-001` |
| P2 | Rewrite `§11` as landed-vs-remaining delta work instead of "add field / add gate" | iOS Architecture | proposal author | Before handoff | current `HEAD` | implementation slices mention only still-unlanded work | `ARCH-002` |
| P2 | Explicitly own stable-doc promotion for watchdog truth in `execution-truth-and-recovery.md` | iOS Architecture | proposal author | Before handoff | stable reference layer | dependency chain does not rely on stale stable docs | `ARCH-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Progress taxonomy | whether weak/strong/mutating semantics are deterministic and shared | classifier mapping and proof-lane expectations match | no silent divergence between proposal and `ExecutionEventBridge` | next proposal review | hold if `§10.2` and shared classifier still disagree |
| Stable execution-truth docs | whether watchdog-specific truth is promoted into stable references | `execution-truth-and-recovery.md` documents `supervisionClassification` and read order | no dual truth between proposal and stable refs | next proposal review | hold if stable dependency chain remains stale |
| Delta framing | whether proposal describes remaining work instead of already-landed work | `§11` references only still-open deltas | no duplicated/phantom implementation steps | next proposal review | hold if `supervisionClassification` and `proposal-037` gate are still described as not yet landed |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: Should `search` / `execute` / `permission:execute` remain weak/discovery progress, or is `P037` intentionally changing them to strong progress?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
