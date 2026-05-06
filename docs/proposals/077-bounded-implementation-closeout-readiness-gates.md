# Proposal 077: Bounded Implementation Closeout Readiness Gates

| Field | Value |
|---|---|
| Date | 2026-05-01 |
| Status | Ready for proposal approval checkpoint (R14) |
| Proposal ID | 077 |
| Revision | 077-bounded-implementation-closeout-readiness-gates-r14 |
| Supersedes | 077-bounded-implementation-closeout-readiness-gates-r13 |
| Source Review Pass | proposal-review-pass-13 |
| Schema | proposal_document_v1 |

> This document is rendered from the approved run-local `proposal_current` artifact for run `d9cdaa6f-07fb-4d07-afaf-0889823c416c`.

## Summary

Introduce one bounded, SQLite-backed closeout-readiness authority before manual release so proposal-backed implementation runs cannot look release-ready only because code self-assessment reports zero blocking code tasks. R14 keeps the approved R13 scope and resolves all pass-13 score-lift feedback by pinning rollout metric sources and owners, enforcement dependency evidence, UI token/control details, recovery escape paths, generation identifier primacy, accessibility throttling, and the control-plane command and transaction choices needed before implementation.

## Problem
- **Constraint:** The fix must fail closed without turning implementation review into an unbounded code/review loop.
- **Failure:** state_9_implementation_reviewed can route toward state_11_manual_release from implementation_self_assessment_v2.blocking_remaining_code_tasks == 0 while proposal proof, audit truth, controlled reports, freshness, risk acceptance, or handoff settlement remains incomplete.
- **Impact:** False readiness weakens operator trust, can advance incomplete proposal implementation, and encourages manual closeout workarounds instead of fixing orchestration and transition behavior.

## Goals
- Require proposal proof, audit truth, controlled evidence, freshness, risk classification, and handoff settlement before manual release in enforcement mode.
- Route code_writer only for code-owned blockers while budget remains; never route non-code handoff to code_writer.
- Expose the same active closeout-readiness generation through transition evaluation, GraphQL, MCP, run-state/exported projections, and macOS readback.
- Keep advisory mode diagnostic-only until release-owner cutover criteria and dependency evidence pass.
- Make every paused, invalid, stale, pending, unknown, or handoff-required state visible, actionable, and accessible to operators.
- Bind rollout decisions to named metric sources, owners, thresholds, and go/no-go actions.

## Non-Goals
- No automated GitHub PR review, Copilot review, or PR comment disposition.
- No auto-approval of human release gates and no replacement of P059 release evidence gates.
- No change to P052 hard loop budget semantics.
- No historical artifact rewrites.
- No new top-level workflow state.
- No SwiftUI non-approval mutations; recovery and settlement remain MCP, CLI, orchestrator, or governed approval work.
- No generalized proposal-gate platform in this pass; ProposalGateExecutor remains P077-scoped until this value is proven.

## Architecture
- **Authority:** implementation_closeout_readiness_v1 is the only enforcement-mode state-9 manual-release authority. Transition code reads SQLite active artifact-contract truth and never exported JSON projections.

### Contracts

#### proposal_gate_result_v1
- **Kind:** active
- **Name:** proposal_gate_result_v1
- **Path:** review/proposal-gate-result.json

##### Statuses
- passed
- failed
- waived
- missing_definition
- stale
- invalid
- unauthorized
- superseded

#### implementation_closeout_readiness_v1

##### Decisions
- enter_manual_release
- return_to_code_refine
- await_non_code_handoff
- await_gate_definition
- await_operator_decision
- **Kind:** active transition authority
- **Name:** implementation_closeout_readiness_v1
- **Path:** review/implementation-closeout-readiness.json

##### Statuses
- ready
- ready_with_risks
- handoff_required
- not_ready
- blocked
- invalid
- unknown

#### implementation_closeout_inputs_v1
- **Kind:** derived diagnostic
- **Name:** implementation_closeout_inputs_v1
- **Path:** review/implementation-closeout-inputs.json
- **Transition Source:** false

#### closeout_handoff_status_v1
- **Kind:** derived operator projection
- **Name:** closeout_handoff_status_v1
- **Path:** review/closeout-handoff-status.json
- **Transition Source:** false

### Decision Matrix
- ready requires implemented audit, current passed or waived gate, green controlled reports, zero code blockers, and no unaccepted risks.
- ready_with_risks enters manual release only when each risk has typed accepted lineage or governed settlement.
- code blockers with budget return to refine.
- code blockers with exhausted budget await operator decision.
- handoff, waiver, rollout, release-owner, or non-code settlement without code blockers becomes handoff_required or enter_manual_release depending on settlement state.
- missing proposal gate records await_gate_definition.
- malformed, stale, unauthorized, or unavailable active inputs fail closed with diagnostic_reason.

### Fingerprint

#### Excluded
- derived inputs/readiness/handoff projections
- exported JSON
- GraphQL projections
- MCP projections

#### Included
- proposal or freeze digest
- run/stage ids
- workflow digest
- worktree HEAD
- dirty/changed-file digest
- upstream active generation ids
- contract version
- **Latency Rule:** Exceeding accepted latency writes closeout_fingerprint_unavailable and fails closed until a current fingerprint is available or waived by governed authority.

### Gate Cause Routing
- **Failed Code Owned Budget Remaining:** return_to_code_refine
- **Failed Unclear Or Budget Exhausted:** await_operator_decision
- **Missing Definition:** await_gate_definition
- **Stale Superseded Or Mismatched:** rerun or import current governed receipt
- **Unauthorized:** await_operator_decision or reject unmanaged receipt
- **Waived Current Fingerprint:** continue matrix with waiver lineage

### Gate Command Surface
- **Decision:** Use one governed action-enum command for proposal gate settlement with actions execute, import_receipt, and waive.
- **Rejection Rule:** Unmanaged file-only receipts are rejected as unauthorized or invalid active inputs.

#### Required Lineage
- principal
- capability
- journal_id
- authority
- reason
- source_artifacts
- run_id
- proposal_id
- stage_id
- workflow_digest
- worktree_HEAD
- dirty_or_changed_file_digest
- source_generation_ids
- current_fingerprint
- **Ui Boundary:** SwiftUI can deep-link, copy a command, or surface a governed approval affordance; it does not perform receipt import or waiver mutation directly.

### Implementation Order
- Domain contracts, parser/domain-status fixtures, and readiness-mode accessor/migration.
- Governed gate-settlement command capability and journal plumbing.
- P077 ProposalGateExecutor.
- State-9 closeout transaction helper and synthesizer.
- Transition guard in state_9/state_10/state_11.
- GraphQL, MCP, run-state, and exported projections through CloseoutReadinessSummaryAccessor.
- macOS read-only surfaces and approval/deep-link/copy affordances.
- scripts/test-gate.sh proposal-077|p077 and docs/reference/test-gates.md.
- **Parser Vs Domain Status:** Malformed or schema-invalid payloads are contract-invalid and do not become active. Well-formed fail-closed domain statuses such as readiness invalid/unknown/blocked and gate missing_definition/stale/unauthorized/failed are valid active generations.

### Proposal Gate Executor

#### Inputs
- run/proposal/stage ids
- gate alias/version
- workflow digest
- worktree HEAD
- dirty/changed-file digest
- source generations
- timeout
- settlement action

#### Outputs
- validated receipt
- evidence/stdout/stderr digests
- timing
- exit code
- executor version
- authorization lineage
- **Scope:** P077-scoped executor only.

### Readiness Mode Storage

#### Allowed Values
- advisory
- enforcement
- **Compatibility Fallback:** workflow_snapshot_json may be read only through the domain accessor for legacy runs; the accessor returns advisory for missing legacy metadata unless an explicit enforcement migration record exists.
- **Decision:** Add a nullable run-owned closeout_readiness_mode column populated from workflow snapshot metadata at run admission. The value is frozen for the run.
- **Fail Closed Rule:** Unknown, malformed, or conflicting mode values are valid diagnostic states but cannot enter manual release without decision enter_manual_release.

#### Tests
- all consumers read the same frozen mode
- advisory mode has no transition side effects
- enforcement mode requires enter_manual_release
- fallback snapshot reads cannot bypass the accessor
- in-flight mode stability survives workflow edits

### Risk Lineage

#### Accepted Sources
- typed controlled risk rows
- release-owner decision record
- governed waiver or settlement command

#### Required Fields
- risk_id
- title
- classification
- authority
- journal_or_decision_id
- source_generation_ids
- settled_at
- **Rule:** Free-form known_risks text never satisfies enter_manual_release.
- **Single Accessor:** CloseoutReadinessSummaryAccessor is the only typed accessor for transitions, GraphQL runs.get/list, MCP runs.get/list, run-state/exported projections, and macOS readback. No consumer parses review/implementation-closeout-readiness.json for transition truth.

### State 9 Sequence
- Import controlled review inputs.
- Execute, import, or waive the proposal gate through the governed command path.
- Call synthesize_implementation_closeout_readiness_for_state9 as a mandatory engine function before transition evaluation.
- Write proposal_gate_result_v1 and implementation_closeout_readiness_v1 active generations in one closeout transaction.
- Rebuild derived projections from the committed active pair.
- Evaluate state transitions only after the closeout transaction commits.

### State 9 Transaction Api
- **Crash Semantics:** Crash before commit leaves previous active truth authoritative. Crash after commit exposes a coherent gate/readiness pair and projection rebuild can be retried from active truth.
- **Decision:** Add a small state-9 closeout repository helper that activates gate and readiness generations, persists summary rows, rebuilds projections once, commits, and only then returns data to transition evaluation.
- **Regression:** A proposal-077 regression must prove no transition is evaluated between gate activation and readiness activation.

## UX and UI Notes

### Applicability
- **Not Applicable Runs:** Render a single neutral Summary row: Closeout readiness not applicable for this run. Tooltip links to the explainer.
- **P077 Compatible Runs:** Render Closeout Readiness in Summary and compact header when state_9 closeout readiness applies.
- **Boundary:** macOS is read-only for closeout readiness except existing approval flows. Recovery writes, receipt imports, waivers, and settlements happen through governed MCP/CLI/orchestrator/approval paths.

### Interaction Rules
- **Compact Activation:** Tap or Return on the compact capsule expands Summary when collapsed, scrolls the Closeout Readiness card into view when available, and focuses the primary unblock. If Summary is unavailable, open the diagnostic sheet directly with focus on the primary unblock.
- **Copy Announcements:** Copy success is polite. Copy failure with fallback is polite and moves focus to the fallback command row. Copy failure without fallback is assertive.
- **Dynamic Announcements:** Collapse same-field changes within three seconds into one announcement, suppress polite refresh announcements while a sheet owns focus, and never announce twice for the same generation hash. Newly blocking enforcement and authority denial remain assertive.
- **Explainer Access:** The advisory/enforcement mode cue is always keyboard-reachable and opens the explainer after first dismissal. A header info button is used when the mode cue is hidden for ready states.
- **Focus Return:** Diagnostic dismissal returns to the trigger. Copy fallback returns to the command row. Approval refresh and pane round trip return to primary unblock.
- **Generation Copy Controls:** Freshness badges expose a context-menu item Copy generation id. The diagnostic sheet header exposes an inline copy icon button. Both controls use the same success, fallback, and failure announcements.
- **Generation Identifier:** The 8-character generation hash is the single operator-facing identifier in tooltip, sheet header, VoiceOver announcement, and copy-to-clipboard. The integer counter is only a relative ordering hint. The raw id is available only through Copy generation id.
- **Secondary Blockers:** Secondary rows are keyboard-focusable, ordered after the primary unblock, styled as queued, and announce: Queued behind {primary_unblock_title}. The diagnostic sheet repeats the gating relationship in body text.
- **Placement:** For degraded state_9 readiness, Closeout Readiness is pinned to Summary. The compact header mirrors only primary reason, mode, and freshness. Secondary evidence deep-links to Diagnostics or Artifacts with a reusable Return to Closeout Readiness backlink.

### Recovery Lifecycle
- old blocker/generation remains visible
- command copied or request prepared
- governed-channel acknowledgement with timestamp and correlation id when available
- external action pending or unknown
- stalled recovery when freshness budget is exceeded
- receipt, waiver, or settlement imported
- readiness refresh in flight
- new active generation replaces blocker or stale/unavailable remains

### Stalled Recovery Affordance
- **A11Y:** Recovery stalled for {command_label}; acknowledged {elapsed} ago; actions available.

#### Actions
- Re-copy command
- Request governed re-issue via Approvals
- Escalate to {handoff_owner}
- **Copy Template:** Still waiting on {command_label}; acknowledged {elapsed} ago.
- **Focus Return:** After action completion or dismissal, return focus to the stalled row.
- **Row:** Non-dismissible inline row below the primary unblock.

### State Matrix

#### ready
- **A11Y:** Ready, updated {time}, generation {hash}
- **Affordance:** open existing approval or refresh
- **Focus:** primary approval affordance

##### Mode Copy
- **Advisory:** Advisory check passed
- **Enforcement:** Ready for manual release
- **Primary Unblock:** all evidence current and green
- **State:** ready

#### ready_with_risks
- **A11Y:** Ready with accepted risks, updated {time}, generation {hash}
- **Affordance:** existing approval only when lineage is current
- **Focus:** risk lineage chip
- **Journey:** accepted

##### Mode Copy
- **Advisory:** Advisory check passed with accepted risks
- **Enforcement:** Ready with accepted risks
- **Primary Unblock:** accepted risk lineage current
- **State:** ready_with_risks

#### ready_with_risks
- **A11Y:** Risk acceptance required for {owner}, generation {hash}
- **Affordance:** handoff-styled settlement-owner row; no approval affordance
- **Focus:** settlement owner row
- **Journey:** acceptance_required

##### Mode Copy
- **Advisory:** Advisory risk-settlement check
- **Enforcement:** Risk acceptance required
- **Primary Unblock:** settlement owner must accept or reclassify risks
- **State:** ready_with_risks

#### handoff_required
- **A11Y:** Handoff required for {owner}, generation {hash}
- **Affordance:** Approvals deep link, governed command copy, or escalation summary
- **Focus:** handoff owner row

##### Mode Copy
- **Advisory:** Advisory handoff check
- **Enforcement:** Handoff required
- **Primary Unblock:** named owner must settle handoff
- **State:** handoff_required

#### not_ready
- **A11Y:** Code work required, {count} blockers, generation {hash}
- **Affordance:** diagnostic only; transition routes to refine when budget remains
- **Focus:** primary code blocker

##### Mode Copy
- **Advisory:** Advisory code-work check
- **Enforcement:** Code work required
- **Primary Unblock:** code blockers remain
- **State:** not_ready

#### blocked
- **A11Y:** Operator decision required, generation {hash}
- **Affordance:** Approvals deep link, governed command copy, or escalation summary
- **Focus:** decision owner row

##### Mode Copy
- **Advisory:** Advisory operator-decision check
- **Enforcement:** Operator decision required
- **Primary Unblock:** budget, authority, or settlement exhausted
- **State:** blocked

#### invalid
- **A11Y:** Evidence invalid: {diagnostic_reason}, generation {hash}
- **Affordance:** diagnostic repair/import/reject path
- **Focus:** diagnostic trigger

##### Mode Copy
- **Advisory:** Advisory evidence invalid
- **Enforcement:** Closeout evidence invalid
- **Primary Unblock:** input invalid or unauthorized
- **State:** invalid

#### unknown
- **A11Y:** Evidence unavailable: {diagnostic_reason}, generation {hash}
- **Affordance:** refresh or diagnostic
- **Focus:** refresh or diagnostic trigger

##### Mode Copy
- **Advisory:** Advisory evidence unavailable
- **Enforcement:** Closeout evidence unavailable
- **Primary Unblock:** active evidence or freshness unavailable
- **State:** unknown

### Visual System
- **Backlink:** Use one reusable leading toolbar button with chevron.left and label Closeout Readiness in Diagnostics and Artifacts. Activation routes to Summary and focuses primary unblock; Return/Escape mirrors this only where the existing app pattern supports it.
- **Banners:** Show at most two non-dismissible banners. Sort blocking/invalid first, then handoff_required, unknown/stale, ready_with_risks, advisory; within same severity sort newest active generation first. Overflow DisclosureGroup preserves the same order.
- **Compact Breakpoint:** Use the existing pane-width compact token if present; otherwise introduce ForgeBreakpoint.closeoutCompactMinimum = 320pt. Transition swaps instantly rather than animating. Primary reason truncates after one line with tooltip, freshness stays visible, and mode cue keeps a 44pt hit target.
- **Contrast:** Phase 0 must measure readyWithRisks and amber fallback candidates on cardElevated and compactCapsule in standard, High Contrast, Reduce Transparency, and Differentiate Without Color modes before advisory rollout.
- **Ready With Risks Split:** Accepted uses a lineage chip with source, authority, timestamp, and typed risk titles beneath it plus Show all overflow. Acceptance-required uses a handoff-styled settlement-owner row, distinct SF Symbol, distinct accessibility label, and no approval affordance.
- **Secondary Blocker Row:** Use supporting typography, queued icon, tertiary or disabled action treatment, visible focus ring, tooltip Awaiting {primary_unblock_label}, and stable order by severity then source generation.
- **Token Mapping Required:** Implementation must add or cite one table mapping every readiness tone, typography style, and breakpoint to current Forge design primitives before SwiftUI work starts.

#### Tone Mapping

##### Item 1
- **Icon:** checkmark.circle
- **Readiness Tone:** readyTone
- **Target:** existing ForgeStatusColor/StatusCapsule success treatment or new approved ForgeStatusTone.ready

##### Item 2
- **Icon Acceptance Required:** exclamationmark.shield
- **Icon Accepted:** checkmark.shield
- **Readiness Tone:** readyWithRisksTone
- **Target:** existing warning/success hybrid only if contrast passes Phase 0; otherwise new approved ForgeStatusTone.readyWithRisks

##### Item 3
- **Icon:** person.crop.circle.badge.exclamationmark
- **Readiness Tone:** handoffTone
- **Target:** existing attention/handoff StatusCapsule treatment or new approved ForgeStatusTone.handoff

##### Item 4
- **Icon:** xmark.octagon
- **Readiness Tone:** blockingTone
- **Target:** existing error/blocking ForgeStatusColor treatment

##### Item 5
- **Icon:** questionmark.circle
- **Readiness Tone:** neutralUnavailableTone
- **Target:** existing neutral/unavailable StatusCapsule treatment

##### Item 6
- **Icon:** progress
- **Readiness Tone:** pendingFirstGenerationTone
- **Target:** neutralUnavailableTone plus leading ProgressView and pending freshness badge

#### Transient Empty States
- **Awaiting First Generation:** Label Awaiting first readiness check, leading ProgressView, pending freshness badge, VoiceOver: Closeout readiness pending first synthesis.
- **Not Applicable:** Neutral Summary row with explainer tooltip.
- **Refresh In Flight:** Keep current tone, add leading ProgressView, and mark freshness as refreshing.
- **Stale Evidence:** Name the changed fingerprint input and route to rerun/import current receipt.
- **Stale Projection:** Keep last tone with stale badge and changed source fingerprint label.
- **Typography Mapping:** Use current ForgeTypography equivalents for section/card title, body, supporting text, micro/status capsule. Do not introduce view-local font constants.

## Metrics

### Diagnostic
- decision/gate distributions
- legacy route disagreement
- latency
- operator recovery usage
- neutral observations
- accessor parity
- education/a11y counters

### Primary
- false_ready_prevented
- post_release_closeout_gap_reversals
- false_blocks
- pause_to_action
- code_writer_loops_avoided

## Rollout

### Decision Payload
- decision_type
- rationale
- cohort
- eligible_closeouts
- primary metric values
- diagnostic metric snapshot
- dependency_checklist_snapshot_id
- fingerprint_p95_threshold_ms
- measurement_window
- waivers
- next_review_date
- readiness links

### Dependency Evidence Checklist

#### Item 1
- **Dependency:** P052 loop budget
- **Fallback:** advisory only
- **Owner:** orchestration owner
- **Pass Rule:** code blockers return to refine only while budget remains; repeated identical blockers trigger soft convergence checkpoint
- **Proof:** route and budget regression fixtures
- **Waiver Authority:** release owner

#### Item 2
- **Dependency:** P059 release evidence gates
- **Fallback:** await_operator_decision
- **Owner:** release owner
- **Pass Rule:** green or governed waived evidence is available to synthesizer
- **Proof:** controlled report source generations and release evidence gate readback
- **Waiver Authority:** release owner

#### Item 3
- **Dependency:** P073 stability freeze
- **Fallback:** block implementation start
- **Owner:** proposal owner
- **Pass Rule:** implementation handoff cites current source truth
- **Proof:** R14 freeze digest or approved run-local artifact citation
- **Waiver Authority:** proposal owner plus release owner

#### Item 4
- **Dependency:** P017 governed command path
- **Fallback:** no receipt import/waiver in enforcement
- **Owner:** control-plane owner
- **Pass Rule:** unmanaged receipts rejected; governed execute/import/waive accepted
- **Proof:** capability, journal, principal, authority, and command fixtures
- **Waiver Authority:** security/release owner

#### Item 5
- **Dependency:** GraphQL/MCP/readback parity
- **Fallback:** advisory only
- **Owner:** API owner
- **Pass Rule:** same active generation fields across GraphQL, MCP, run-state, and exported projection
- **Proof:** runs.get/list parity fixtures through CloseoutReadinessSummaryAccessor
- **Waiver Authority:** release owner

#### Item 6
- **Dependency:** macOS UI evidence
- **Fallback:** CLI/MCP readback only; no enforcement cutover through UI
- **Owner:** macOS owner
- **Pass Rule:** no overlap, current tokens mapped, and recovery actions remain read-only/deep-link/copy
- **Proof:** state matrix, transient, compact, a11y, focus, copy, and token fixtures
- **Waiver Authority:** release owner plus UX/UI owner

#### Item 7
- **Dependency:** fingerprint p95 threshold
- **Fallback:** write closeout_fingerprint_unavailable and stay advisory
- **Owner:** control-plane owner
- **Pass Rule:** p95 below release-owner threshold before enforcement
- **Proof:** Phase 1 latency snapshot
- **Waiver Authority:** release owner

### Diagnostic Metrics
- decision and gate status distributions
- legacy route disagreement
- synthesis and fingerprint latency
- operator recovery usage
- neutral observations
- accessor parity
- a11y announcement count
- explainer reopen count

### Expansion Criteria
- confirmed avoided false-ready or explicit neutral-observation decision
- zero post-release closeout-gap reversals
- false blocks <= 5% or <= 2 in first cohort
- median pause-to-action < 1 business day unless waived
- no recurring diagnostic bucket > 30% without action plan
- 100% non-code handoff cases avoid code_writer
- dependency checklist passed or explicitly waived with authority
- **First Cohort:** 10 eligible state-9 closeouts or 10 business days for P052/P059/P073-compatible proposal-backed runs.

### Metric Ledger

#### Item 1
- **Denominator:** eligible closeouts
- **Go No Go Action:** continue advisory, limited enforcement, extend cohort, or hold with written rationale
- **Metric:** false_ready_prevented
- **Numerator:** eligible closeouts blocked by P077 where legacy self-assessment path would have allowed manual release
- **Owner:** release owner
- **Source:** closeout readiness decision log plus would-have-entered-manual-release legacy comparison
- **Threshold:** at least one confirmed prevention in cohort, or neutral-observation decision is required

#### Item 2
- **Denominator:** P077-governed manual releases
- **Go No Go Action:** any reversal pauses enforcement expansion and requires corrective action
- **Metric:** post_release_closeout_gap_reversals
- **Numerator:** releases reversed because proposal proof, audit truth, gate freshness, risk settlement, or handoff was incomplete
- **Owner:** release owner
- **Source:** manual release receipts, closeout readiness generations, and post-release reversal/incident records
- **Threshold:** zero for expansion

#### Item 3
- **Denominator:** eligible closeouts
- **Go No Go Action:** breach reverts new runs to advisory within one business day
- **Metric:** false_blocks
- **Numerator:** closeouts blocked by P077 that release owner classifies as incorrect
- **Owner:** control-plane owner
- **Source:** operator override records, release-owner decisions, and readiness diagnostic reasons
- **Threshold:** <= 5% or <= 2 in first cohort

#### Item 4
- **Denominator:** paused closeouts
- **Go No Go Action:** breach requires copy, routing, or ownership fix before expansion
- **Metric:** pause_to_action
- **Numerator:** elapsed business time per paused closeout
- **Owner:** operator experience owner
- **Source:** first blocking readiness generation timestamp to governed acknowledgement, settlement, rerun, or operator decision timestamp
- **Threshold:** median < 1 business day unless release owner waives with reason

#### Item 5
- **Denominator:** non-code handoff or operator-decision cases
- **Go No Go Action:** fix routing before enforcement expansion
- **Metric:** code_writer_loops_avoided
- **Numerator:** non-code handoff or operator-decision cases that did not invoke code_writer
- **Owner:** orchestration owner
- **Source:** decision route, blocker classification, and code_writer invocation records
- **Threshold:** 100% expected; any regression blocks expansion
- **Neutral Observation Rule:** If no avoided-false-ready opportunity appears and all thresholds are green, release owner must choose continue advisory, limited enforcement, extend cohort with date, or hold with rationale. There is no silent expansion.

### Phases
- **0:** Admission and implementation handoff: confirm P052/P059/P073/P017 dependencies, freeze R14 source, add contracts, mode storage, command surface, transaction helper, metric ledger, dependency checklist, token mapping, and contrast measurements.
- **1:** Advisory for new eligible runs: synthesize/read back readiness, show pause reason and explainer, collect disagreement and metric-source snapshots, and make no transition changes.
- **2:** Release-owner enforcement after parity evidence, v2 decode, paused journey fixtures, frozen mode, current UI evidence, dependency checklist pass, fingerprint p95 threshold, and rollback plan are all approved.
- **Rollback:** False-block threshold breach or closeout-gap reversal reverts new runs to advisory within one business day. In-flight modes stay frozen unless explicitly migrated by governed decision.

## Acceptance

### Excluded Validation
- live Xcode
- local UI smoke tests
- simulator
- GitHub PR review
- Copilot review
- daemon dogfood
- benchmarks
- load tests
- fuzzing
- network access

### Implementation Handoff Required
- R14 or approved freeze digest
- metric ledger
- dependency evidence checklist
- design token mapping table
- state-9 closeout transaction helper
- governed gate-settlement command shape
- readiness-mode storage migration/accessor decision
- current UI review or explicit carry-forward

### Proof Gate
- Register scripts/test-gate.sh proposal-077|p077 and docs/reference/test-gates.md.
- A complete self-assessment with a missing proposal gate records await_gate_definition and cannot enter manual release.
- Audit Not Ready with code-owned blockers and remaining loop budget returns to implementation refine.
- Audit Not Ready with no code-owned blockers does not invoke code_writer and records handoff or operator-decision state.
- Ready with Risks enters manual release only with typed accepted risk lineage, governed waiver, rollout constraint, follow-up, or release-owner decision.
- Green active proposal gate, green controlled reports, current audit truth, zero code blockers, and settled risks enter manual release.
- Repeated identical blockers trigger a soft convergence checkpoint without claiming P052 hard loop budget exhaustion.
- Transition evaluation ignores stale exported JSON and reads only active SQLite artifact-contract truth.
- GraphQL and MCP expose the same CloseoutReadinessSummaryAccessor fields for runs.get and runs.list.
- macOS fixtures cover ready, ready_with_risks accepted, ready_with_risks acceptance_required, handoff_required, not_ready, blocked, invalid, unknown, awaiting_first_generation, refresh-in-flight, stale projection, stale evidence, and not-applicable.
- Accessibility fixtures prove bounded VoiceOver announcements during rapid refresh, keyboard access to secondary blockers, copy-generation controls, backlink routing, and re-openable explainer access.

## Risks

### Item 1
- **Mitigation:** One active readiness authority; inputs and projections are diagnostic or derived only.
- **Risk:** Parallel release policy

### Item 2
- **Mitigation:** Governed action-enum command with capability, journal, principal, authority, reason, source artifacts, and fingerprint lineage; unmanaged receipts rejected.
- **Risk:** Gate import side channel

### Item 3
- **Mitigation:** Frozen run-owned mode, visible mode cue, re-openable explainer, and advisory no-side-effect tests.
- **Risk:** Advisory/enforcement confusion

### Item 4
- **Mitigation:** Advisory cohort, explicit thresholds, dependency checklist, owner decisions, and one-business-day rollback.
- **Risk:** False blocking

### Item 5
- **Mitigation:** Token mapping, shared fixture matrix, reusable backlink, deterministic banner ordering, and a11y/focus fixtures.
- **Risk:** UI drift across Summary, compact header, Diagnostics, and Artifacts

### Item 6
- **Mitigation:** Typed risk rows and governed settlement lineage are required; free-form known_risks text never releases.
- **Risk:** Risk acceptance inferred from prose

### Item 7
- **Mitigation:** State-9 closeout transaction helper commits active gate/readiness together before transitions.
- **Risk:** Atomicity gap between gate and readiness activation

### Item 8
- **Mitigation:** Phase 0 contrast measurement is a cutover gate with named surfaces and candidate fallback tokens.
- **Risk:** Contrast values unknown before implementation

## Feedback Resolution

### Addressed
- PO-R13-001
- PO-R13-002
- UX-077-R13-01
- UX-077-R13-02
- UX-077-R13-03
- UX-077-R13-04
- UX-077-R13-05
- UX-077-R13-06
- UX-077-R13-07
- UX-077-R13-08
- UI-077-R13-01
- UI-077-R13-02
- UI-077-R13-03
- UI-077-R13-04
- UI-077-R13-05
- UI-077-R13-06
- UI-077-R13-07
- UI-077-R13-08
- ARCH-P077-R13-01
- ARCH-P077-R13-02
- ARCH-P077-R13-03

### Deferred Or Disputed

#### CONTRAST-MEASURED-RATIOS
- **Id:** CONTRAST-MEASURED-RATIOS
- **Resolution:** Not a proposal-time factual claim. R14 names exact surfaces and modes; Phase 0 measurement is a cutover gate.

#### PROPOSAL-GATE-GENERALIZATION
- **Id:** PROPOSAL-GATE-GENERALIZATION
- **Resolution:** Intentionally deferred. P077 remains scoped to avoid broad migration cost before value proof.

### Unresolved External

_None._

## Open Questions

### Should ProposalGateExecutor generalize beyond P077 after this proves value?
- **Question:** Should ProposalGateExecutor generalize beyond P077 after this proves value?
- **Status:** deferred_until_after_p077_enforcement_evidence

### Should implementation_review_summary_v1 remain as a historical compatibility projection?
- **Question:** Should implementation_review_summary_v1 remain as a historical compatibility projection?
- **Status:** keep_until transition, GraphQL, MCP, run-state, and macOS parity evidence exists

### Should fingerprint p95 be workflow-wide or release-owner configurable after Phase 1?
- **Question:** Should fingerprint p95 be workflow-wide or release-owner configurable after Phase 1?
- **Status:** decide from Phase 1 latency snapshot before Phase 2

## Source Truth
- **Authoritative:** This run-local R14 proposal_current artifact, or an explicitly approved freeze digest derived from it.
- **Checked In Path:** docs/proposals/077-bounded-implementation-closeout-readiness-gates.md
- **Implementation Handoff Rule:** Implementation tasks must cite the R14 artifact or approved freeze digest. Stale checked-in proposal text is not an implementation source.
- **Review Context:** R14 is based on proposal-review-pass-13, which reported zero blockers, aggregate score 8.375, and 21 non-blocking score-lift items.

## Runtime Provenance
- **Agent Execution Id:** e8d9cf9f-698b-4e6b-859a-9222b19b5279
- **Generated At:** 2026-05-01
- **Run Id:** d9cdaa6f-07fb-4d07-afaf-0889823c416c
- **Session Generation Id:** 7dbd9dda-58b2-4e50-87d4-fa45308f54ec
- **Session Reuse Disposition:** fresh_after_budget
- **Stage Execution Id:** d3377df3-234b-42bb-88e8-7c0904c11241
- **Stage Id:** state_5_proposal_refined
- **Work Item Id:** p058-invoke:d3377df3-234b-42bb-88e8-7c0904c11241:0
