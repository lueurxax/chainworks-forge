# Proposal 008: MVP Hardening and Sign-Off Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `e63d440` |
| Reviewed At | `2026-03-24T21:29:32+0200` |
| Review Mode | `full-review` |
| Product Overlay | `included` |
| Overall Status | `Reviewed` |
| Readiness | `Red` |
| Confidence | `Medium` |
| Evidence Completeness | `Partial` |

## 0. Review Mode and Evidence Summary

- Mode used: `full-review`
- Evidence completeness: `Partial`
- Documents / repo inputs reviewed:
  - `docs/proposals/008-mvp-hardening-and-sign-off.md`
  - `docs/ps/chainworks-forge-mvp.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reviews/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding-review.md`
- Build/run attempts:
  - `RUN-01` current-head macOS build passed
  - `RUN-02` approval inbox UI proof passed
  - `RUN-03` missing-runtime guidance UI proof passed
  - `RUN-04` relaunch-at-approval UI proof passed
  - `RUN-05` earlier broad UI reruns stalled while stale runners were present
- Screenshots captured:
  - `SCR-01` approval inbox
  - `SCR-02` missing-runtime guidance
  - `SCR-03` restored waiting-approval inbox on relaunch
- Code areas inspected:
  - attachment storage and rendering
  - execution-context / Goose packet assembly
  - current operator shell: `RunsHomeView`, `RecoverySheet`, `RunReportView`, `RunComparisonView`, `ForegroundBannerView`, `ContentView`
  - current `Run` model boundary
- Remaining assumptions:
  - current `.xcresult` attachments are acceptable screenshot evidence even though the images were not separately exported
  - for Proposal 008, the relevant primary flow is the current approval/relaunch operator path
- Remaining blockers:
  - current Proposal 007 dependency is still future-state on current HEAD
  - new Proposal 008 surfaces do not exist yet, so UI/UX evaluation is against proposal text plus current shell evidence rather than implemented 008 screens

## 1. Executive Summary

- Overall readiness: `Red`
- Confidence: `Medium`
- Release blockers:
  1. The `GO/HOLD` launch gate is not reproducible from persisted system state yet.
  2. Proposal 008 is sequenced as sign-off work even though Proposal 007 is still not evidenced as complete on current HEAD.
  3. The proposal adds new recovery/export/sign-off surfaces without defining how they stay inside the current operator shell.
- Top risks:
  1. Benchmark/sign-off data will drift into notebook or raw-log arbitration instead of replayable in-app evidence.
  2. The proposal will fragment the operator shell by adding parallel surfaces instead of extending `RunsHomeView` / `RecoverySheet` / `RunReportView`.
  3. Attachment policy will over-promise end-to-end support that the runtime does not currently implement.
- Top opportunities:
  1. Split benchmark/sign-off state from operational `Run` state before implementation begins.
  2. Make the sign-off summary reconstruct the entire `GO/HOLD` decision from stored cohort records.
  3. Turn recovery/export/sign-off into shell-owned routes rather than standalone destinations.

Verdict: Proposal 008 is directionally correct as the post-007 hardening slice, and this round cleared the minimum full-review gate with fresh build evidence plus three targeted UI screenshot bundles. The remaining issues are not evidence gaps in the review process; they are proposal gaps. As written, the draft still leaves the KPI proof model, dependency sequencing, and operator-shell ownership too loose for safe sequential implementation.

## 2. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | Medium | Partial | 0 | 1 | 1 | 1 |
| UX | Red | Medium | Partial | 0 | 1 | 2 | 0 |
| iOS Architecture | Red | Medium | Partial | 0 | 2 | 2 | 0 |
| Product | Red | Medium | Partial | 0 | 2 | 2 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings

- Finding ID: `UI-008-01`
  Severity: `High`
  Evidence IDs: `DOC-01`, `CODE-03`, `BASE-01`, `SCR-01`, `SCR-03`
  Why it matters: Proposal 008 adds `BlockedRunRecoveryView`, `CompletedRunExportHub`, and `MVPSignOffSummaryView`, but it does not define how those surfaces fit into the current shell, which already centers on `RunsHomeView` with recovery/report routed as shell-owned flows. That is a direct fragmentation risk.
  Recommended fix: Rewrite sections 7 and 8 so the new surfaces are explicitly shell-owned extensions of `RunsHomeView` / `RecoverySheet` / `RunReportView`, or document one canonical replacement hierarchy.
  Acceptance criteria: every recovery/export/sign-off state is reachable from one shell entry point; no duplicate top-level destinations exist for the same task; UI test flows enter the new states from the current operator shell.
  Confidence: `Medium`

- Finding ID: `UI-008-02`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-03`, `RUN-02`, `RUN-03`, `RUN-04`
  Why it matters: The completed-run export hub is specified as a content bucket, not a visual hierarchy. The proposal lists report, receipts, evidence-pack status, elapsed time, total cost, breakdowns, and export actions without defining primary emphasis or progressive disclosure.
  Recommended fix: Add explicit visual-order rules: one dominant summary region, one first-class evidence-pack status treatment, and secondary receipt detail behind disclosure groups or subordinate sections.
  Acceptance criteria: the export hub spec names the primary summary block, the evidence-pack status encoding, and the progressive-disclosure rules for receipts and breakdowns.
  Confidence: `Medium`

- Finding ID: `UI-008-03`
  Severity: `Low`
  Evidence IDs: `DOC-01`, `CODE-03`
  Why it matters: Proposal 008 fixes a `p95 <= 2.0s` open-time target but does not specify loading, empty, timeout, or transition states for the new report/export surfaces. That leaves room for blank-screen or flash-state regressions while trying to meet the SLO.
  Recommended fix: Add state-design requirements for loading, empty, timeout, and retry behavior on report/export surfaces.
  Acceptance criteria: each output/report surface has a defined loading state, a timeout/failure affordance, and no blank-content transition path.
  Confidence: `Low`

### 3.2 UX Findings

- Finding ID: `UX-008-01`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-02`, `DATA-01`, `QUESTION-01`
  Why it matters: Proposal 008 defines a hard `GO/HOLD` gate but does not specify a user-legible summary of cohort membership, manual baseline, app result, median calculation, or reason codes for `HOLD`. That makes the key launch decision too easy to arbitrate outside the app.
  Recommended fix: Require `MVPSignOffSummaryView` and the exported packet to show cohort members, per-run timings, manual-vs-app pairings, median calculation, and explicit failing gate conditions.
  Acceptance criteria: a reviewer can reconstruct the `GO/HOLD` result from the app/export packet alone, without notebook state or raw-log archaeology.
  Confidence: `Medium`

- Finding ID: `UX-008-02`
  Severity: `Medium`
  Evidence IDs: `CODE-03`, `BASE-01`, `BASE-02`, `QUESTION-03`, `RUN-04`, `SCR-03`
  Why it matters: The proposal does not say whether recovery/export surfaces extend or replace the current operator shell, even though `waiting_approval` restoration already exists there. Hidden or duplicated routing will make recovery harder exactly where 008 is supposed to reduce ambiguity.
  Recommended fix: Document the canonical entry path from the current shell into blocked-run recovery and completed-run export, including return behavior and context preservation.
  Acceptance criteria: from a selected run, recovery/export is one predictable path; returning preserves the same run context with no rediscovery work.
  Confidence: `Medium`

- Finding ID: `UX-008-03`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-01`, `CODE-02`, `QUESTION-02`
  Why it matters: The attachment policy reads as supported runtime behavior, but current evidence shows attachments are only stored as local-path references. If the UI does not distinguish “reference only” from “actually ingested by the run,” operator trust will degrade.
  Recommended fix: Specify UI language and validation states that distinguish reference-only attachments from agent-ingested inputs.
  Acceptance criteria: before launch, every attachment is clearly labeled by runtime role; unsupported or reference-only files are explained inline rather than implied as consumed inputs.
  Confidence: `Medium`

### 3.3 iOS Architecture Findings

- Finding ID: `ARCH-008-01`
  Severity: `High`
  Evidence IDs: `DOC-01`, `CODE-04`
  Why it matters: Proposal 008 plans to put benchmark timing fields, evidence-pack status, and MVP sign-off state directly onto `Run`, but `Run` is already the operational execution aggregate. That collapses runtime state, benchmark state, and launch-governance state into one persistence boundary.
  Recommended fix: Introduce a separate benchmark/sign-off aggregate linked to `Run` IDs instead of extending `Run` with launch-gate concerns.
  Acceptance criteria: sign-off data can evolve without changing the operational `Run` lifecycle; sign-off queries are computed from the benchmark aggregate rather than workflow state mutation.
  Confidence: `Medium`

- Finding ID: `ARCH-008-02`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DATA-01`, `QUESTION-01`
  Why it matters: The `GO/HOLD` evaluator depends on manual-vs-app pairs and checkpoint timings, but the proposal does not define a persisted pairing/provenance model or import boundary for manual baseline runs. Without that, the evaluator will drift into notebook-driven logic.
  Recommended fix: Add first-class persisted entities for benchmark cohort, manual/app pair ID, measurement records, and raw artifact links, and require `MVPSignOffEvaluator` to consume only those records.
  Acceptance criteria: rerunning the evaluator on the same stored records yields the same result; every manual/app pair is queryable by cohort ID; no raw-log archaeology is needed to justify the final gate.
  Confidence: `Medium`

- Finding ID: `ARCH-008-03`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-01`, `CODE-02`
  Why it matters: Proposal 008 freezes a supported attachment-type list without defining the runtime resolution boundary that validates, rejects, or converts those files into execution inputs. Current runtime evidence does not carry `attachmentPath` into execution context at all.
  Recommended fix: Add an attachment-resolution step before agent submission that validates file type/path and emits either an execution artifact or a deterministic rejection record.
  Acceptance criteria: supported attachment types are validated exactly once, passed into execution or rejected deterministically, and covered by tests.
  Confidence: `High`

- Finding ID: `ARCH-008-04`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-03`
  Why it matters: The proposal adds new views without assigning routing ownership inside the current shell. That is a state-management and navigation-coherence risk, not only a UI polish issue.
  Recommended fix: Keep one shell owner for run detail, recovery, report, export, and sign-off routes; implement new surfaces as shell-owned subroutes/subviews.
  Acceptance criteria: one selected-run source of truth drives all new flows; no duplicate modal paths exist for the same action.
  Confidence: `High`

### 3.4 Product Findings

- Finding ID: `PRD-008-01`
  Severity: `High`
  Evidence IDs: `DOC-04`, `BASE-03`, `BLOCKER-01`
  Why it matters: Proposal 008 is framed as the MVP sign-off slice even though current repo evidence still says Proposal 007 is future-state. That makes the sign-off gate premature and risks mixing “finish core repo-backed runtime” with “validate MVP.”
  Recommended fix: Add an explicit prerequisite gate: Proposal 008 starts only after Proposal 007 reaches verified repo-backed completion with current-head evidence.
  Acceptance criteria: 008 text declares 007 completion evidence as a hard prerequisite; no `GO/HOLD` evaluation runs before that dependency is satisfied.
  Confidence: `High`

- Finding ID: `PRD-008-02`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DATA-01`, `QUESTION-01`
  Why it matters: The launch gate is not yet product-credible because the proposal does not fully define how benchmark cohort records, manual baselines, timestamps, and artifact links are captured so the KPI can be replayed from system state alone.
  Recommended fix: Require `BenchmarkRunRecorder` and `MVPSignOffEvaluator` to operate only on persisted cohort/mode/timing/artifact records, with a replayable exported decision payload.
  Acceptance criteria: every benchmark run has a frozen cohort ID, mode, timestamps, and artifact links; the `GO/HOLD` result can be recomputed from exported records without external notes.
  Confidence: `Medium`

- Finding ID: `PRD-008-03`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `DOC-02`
  Why it matters: The new `p95 <= 2.0s` SLO is introduced without measured local latency data or a defined hardware envelope. Right now it reads as an arbitrary promise rather than a validated launch contract.
  Recommended fix: Derive the threshold from measured p50/p95/p99 data on representative artifact sizes and name the hardware class and measurement method in the proposal.
  Acceptance criteria: the proposal includes the benchmark method, hardware profile, artifact-size class, and baseline distribution that justify the chosen SLO.
  Confidence: `Medium`

- Finding ID: `PRD-008-04`
  Severity: `Medium`
  Evidence IDs: `CODE-01`, `CODE-02`, `QUESTION-02`
  Why it matters: Proposal 008 promises a fixed set of attachment types, but current runtime evidence only supports stored path references. That overstates actual MVP value unless the proposal narrows the claim or the runtime expands.
  Recommended fix: Either narrow the policy to “reference-only attachments” for MVP or explicitly add true attachment ingestion to scope.
  Acceptance criteria: product language and runtime behavior match for each supported type, and the chosen policy is proven by end-to-end tests.
  Confidence: `High`

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: product wants a strong MVP sign-off gate, but architecture/UX evidence shows the benchmark data model is not yet replayable.
  Tradeoff: shipping a simple `GO/HOLD` surface quickly versus shipping a trustworthy, auditable one.
  Decision: favor replayability over speed; the sign-off gate must be computed from persisted cohort records only.
  Owner: proposal author

- Conflict: new dedicated recovery/export/sign-off views could simplify implementation, but current shell evidence shows a strong central operator surface already exists.
  Tradeoff: faster isolated view delivery versus coherent operator navigation.
  Decision: keep the current shell as the owner and make the new surfaces subroutes/subviews.
  Owner: proposal author

- Conflict: broad attachment-type support improves headline scope, but runtime evidence only supports stored path references today.
  Tradeoff: broader promise versus truthful MVP contract.
  Decision: either narrow the promise to reference-only behavior or explicitly scope runtime ingestion/validation work.
  Owner: proposal author

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add a persisted benchmark/sign-off model separate from `Run`, including manual/app pair IDs, cohort IDs, timestamps, artifact links, and replayable evaluator inputs | Architecture + Product | Proposal author | Before implementation | None | `GO/HOLD` result is reproducible from exported records only | `ARCH-008-01`, `ARCH-008-02`, `UX-008-01`, `PRD-008-02` |
| P0 | Add an explicit prerequisite gate that Proposal 008 starts only after Proposal 007 has verified repo-backed completion evidence | Product | Proposal author | Before implementation | Proposal 007 evidence refresh | Zero sign-off runs evaluated against future-state-only runtime | `PRD-008-01` |
| P1 | Rewrite sections 7 and 8 so recovery/export/sign-off surfaces are shell-owned extensions of the current operator shell, with canonical routing and back-navigation | UI + UX + Architecture | Proposal author | Before implementation | P0 not required | Every new state is reachable from one shell path in spec and tests | `UI-008-01`, `UX-008-02`, `ARCH-008-04` |
| P1 | Re-scope attachment policy to match runtime reality, or explicitly add attachment-resolution/ingestion scope and tests | Product + UX + Architecture | Proposal author | Before implementation | None | Attachment contract and runtime behavior match for every supported type | `UX-008-03`, `ARCH-008-03`, `PRD-008-04` |
| P2 | Justify the `p95 <= 2.0s` SLO with measured latency data, hardware envelope, artifact-size class, and loading/error-state UX rules | Product + UI | Proposal author | Before sign-off hardening implementation | None | SLO is measurable, reproducible, and paired with explicit UI state behavior | `UI-008-03`, `PRD-008-03` |
| P2 | Add export-hub visual hierarchy rules and progressive disclosure for receipts/breakdowns/evidence-pack status | UI | Proposal author | Before sign-off UI implementation | P1 | Export surface stays scannable under full sign-off payload | `UI-008-02` |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Benchmark replayability | Whether `GO/HOLD` can be recomputed from persisted cohort records only | % benchmark runs with frozen cohort/mode/timing/artifact records | Decision replay mismatch rate must stay `0` | Before exposing sign-off gate | Hold if any sign-off decision requires notebook or raw-log arbitration |
| Dependency sequencing | Whether 007 prerequisites are evidenced before 008 sign-off work starts | % prerequisite capabilities with current-head evidence | Zero sign-off runs before 007 proof | Roadmap sequencing review | Hold if any 008 milestone depends on future-state-only 007 runtime |
| Operator-shell routing | Number of entry points to recovery/export/sign-off states | UI test reachability from current shell | No duplicate top-level route for same action | Spec review before implementation | Hold if routes fragment across parallel destinations |
| Attachment contract | Runtime treatment of each declared supported attachment type | % supported types with deterministic validation + execution/rejection behavior | Zero implied ingestion for reference-only files | Attachment-policy freeze | Hold if product language exceeds runtime behavior |
| Output/report latency | p50/p95/p99 open time for representative artifacts on named hardware | p95 open time | No blank-state flash or unrecoverable timeout | SLO freeze review | Hold if threshold is not backed by measured data |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: No implemented Proposal 008 surfaces exist yet for direct simulator review of `BlockedRunRecoveryView`, `CompletedRunExportHub`, or `MVPSignOffSummaryView`.
- `GAP-02`: No real benchmark cohort records or launch-gate outputs exist yet, so the KPI proof remains proposal-level rather than implementation-verified.
- `GAP-03`: No measured latency dataset exists yet to validate the proposed `p95 <= 2.0s` SLO.

### Open Questions

- `QUESTION-01`: What concrete persisted schema represents manual baseline runs and pairs them to app-driven runs?
- `QUESTION-02`: Are non-text attachments supposed to be reference-only in MVP, or actually consumable by the runtime?
- `QUESTION-03`: Which current shell owner is authoritative for recovery/export/sign-off routing: `RunsHomeView`, `RecoverySheet`, or a new route owner?
