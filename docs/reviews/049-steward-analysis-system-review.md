# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Reviewed on: `2026-04-15`
- Reviewed tree: working tree with local modifications in `docs/proposals/049-steward-analysis-system.md`
- Proposal / docs reviewed:
  - `docs/proposals/049-steward-analysis-system.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/forge-steward.md`
  - prior `049` review / evidence artifacts as stale-basis comparators
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/forge-steward.md`
- Baseline refreshed:
  - targeted reread of the current `P049` draft and working-tree diff
  - targeted reread of stable Swift Steward V1 dossier / context-link behavior
  - targeted code refresh for current Swift run-owned cohort / provenance ownership
  - targeted code refresh for current Rust daemon startup, run command, run schema, work-item queue, and DB migration baseline
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift`
  - `control-plane/crates/domain/src/run.rs`
  - `control-plane/crates/domain/src/commands.rs`
  - `control-plane/crates/db/src/work_item.rs`
  - `control-plane/crates/daemon/src/main.rs`
  - `control-plane/crates/db/migrations/005_validation_records.sql`
  - `control-plane/crates/db/migrations/006_session_lineage.sql`
- Current repo contradictions found:
  - the old no-signal dossier blocker is stale: the current draft now restores the bounded context-dossier fallback and `context` links
  - the old deterministic-artifact blocker is stale: the current draft now names `BTreeMap` for artifact-visible maps and a canonical JSON writer owner
  - the old daemon-global current-input blocker is stale: the current draft now defines `StewardRuntimeInputs`, daemon-owned config/catalog paths, and the shared queue path
  - the current draft now aligns with the stable Swift run-owner model for Steward cohort / frozen provenance fields
  - one live repo-baseline contradiction remains: the file inventory still asks for `db/migrations/005_steward.sql`, but the current Rust schema chain already uses `005_validation_records.sql` and `006_session_lineage.sql`
  - one low-level internal inconsistency remains: `AnomalyDetector.detect(...)` still takes `HashMap<String, ThresholdEntry>` while the proposal moved `StewardConfig.thresholds` to `BTreeMap<String, ThresholdEntry>`
- Remaining blockers:
  - migration numbering / file-inventory collision with the current DB baseline

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong; the stale red basis is closed, but one repo-baseline collision and one low-level type mismatch remain`
- Top residual implementation risks:
  1. Literal implementation of `db/migrations/005_steward.sql` would clash with the existing migration chain and force ad hoc renumbering outside the proposal text.
  2. The lingering `HashMap` / `BTreeMap` split around thresholds weakens the proposal's otherwise tightened deterministic-contract story.
  3. If the migration fix is made in one section only, the proposal can still hand implementation an inconsistent file-inventory / proof-plan story.
- Top opportunities:
  1. Rename the Steward migration to the next free slot and make the migration strategy explicit.
  2. Normalize the thresholds container type across all code snippets.
  3. Carry the now-correct run-owner, daemon-current-input, and canonical-writer contracts into implementation without reopening the older stale blockers.

## 2. Proposal Scope and Completeness
- In scope:
  - Rust Steward V1 parity for metrics, anomaly detection, cohorting, dossiers, persisted analyses, and recommendations
  - both optional LLM lanes
  - post-run, config-change, and manual trigger wiring
  - run-owned cohort / provenance bridge and daemon-owned current-input chain
- Out of scope:
  - Steward UI
  - V2 recommender / V3 experimenter
  - runtime consumption of `context_strategy_profiles`
  - implementation audit or gate execution
- Most important baseline refreshes performed:
  - stable Swift dossier fallback and `context` link behavior
  - stable Swift run-owned cohort / frozen provenance ownership
  - current Rust daemon startup / work-item / migration baseline
- Most important contradictions with current repo:
  - the only live repo-baseline contradiction is the proposed migration filename / sequence
- Most important missing or partial states:
  - thresholds container typing is still internally inconsistent across the draft

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
| Architecture | Amber | High | Complete | 0 | 0 | 1 | 1 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` findings.

### 5.2 UX Findings
- No live UX `proposal-text` findings.

### 5.3 Architecture Findings

#### ARCH-001 - Proposed Steward migration collides with the current Rust migration chain
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `MAP-08`, `INT-04`, `REAL-04`
- Proposal refs:
  - `docs/proposals/049-steward-analysis-system.md:585`
- Current repo refs:
  - `control-plane/crates/db/migrations/005_validation_records.sql:1`
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1`
- Why it matters:
  - The draft now does the important architecture work correctly, but its file inventory still instructs implementers to add `db/migrations/005_steward.sql`. In the current tree, `005` and `006` are already occupied. That makes the migration plan non-executable as written and pushes a real schema-ordering decision out of the proposal and into ad hoc implementation judgment.
- Required fix:
  - Rename the Steward migration in the proposal to the next free slot in the current chain, or state explicitly that implementation must use the next available migration number at landing time without renumbering existing migrations.
  - Update every place that names the migration so the file inventory, implementation plan, and proof expectations stay aligned.
- Acceptance criteria:
  - The proposal no longer names an already-occupied migration slot.
  - The migration strategy explicitly preserves the existing `001` through `006` sequence.

#### ARCH-002 - Threshold container typing is still internally inconsistent
- Severity: `Low`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `INT-05`, `REAL-05`
- Proposal refs:
  - `docs/proposals/049-steward-analysis-system.md:281`
  - `docs/proposals/049-steward-analysis-system.md:435`
- Why it matters:
  - The current draft correctly moved `StewardConfig.thresholds` to `BTreeMap` as part of the deterministic contract, but `AnomalyDetector.detect(...)` still takes `&HashMap<String, ThresholdEntry>`. That does not reopen the old determinism blocker, but it leaves the proposal internally inconsistent and invites unnecessary conversion glue or a partial retreat from the new type contract during implementation.
- Required fix:
  - Align the `detect(...)` signature and any remaining snippets with the same thresholds container type used by `StewardConfig`.
  - If thresholds are intentionally allowed to stay unordered internally because they are not artifact-visible, state that explicitly instead of mixing both contracts.

## 6. Cross-Discipline Conflicts and Decisions
- No live cross-discipline conflict remains. The residual work is architecture-only cleanup before `Green`.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Fix the Steward migration naming so it targets the next free DB slot instead of `005` | Architecture | proposal author | Before next review | current `db/migrations` baseline | proposal no longer collides with the live migration chain | `ARCH-001` |
| P2 | Normalize thresholds container typing across the proposal text | Architecture | proposal author | Before next review | current deterministic JSON contract wording | proposal no longer mixes `BTreeMap` and `HashMap` for the same thresholds owner boundary | `ARCH-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| migration sequencing | whether the proposal is executable against the live DB baseline | the file inventory names a free migration slot | no renumbering of existing migrations | next proposal review | hold if the draft still names `005_steward.sql` |
| threshold contract consistency | whether the tightened determinism contract stays internally coherent | all threshold-carrying snippets use one container type | no silent `HashMap` reintroduction through helper signatures | next proposal review | hold if `StewardConfig` and `AnomalyDetector` still disagree on the thresholds type |
| implementation handoff stability | whether the now-correct owner chains remain intact | run-owner bridge, daemon current-input source, and canonical writer all stay unchanged in the text | no reintroduction of stale blockers already closed in this draft | implementation audit | hold only on implementation drift, not on already-fixed proposal sections |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: Should the draft name `007_steward.sql` explicitly for the current tree, or should it state "next available migration number at landing time" to stay safe against parallel schema work?
- QUESTION-02: Does the author want thresholds to stay `BTreeMap` end-to-end, or to remain unordered internally with determinism enforced only at artifact-writing boundaries?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
