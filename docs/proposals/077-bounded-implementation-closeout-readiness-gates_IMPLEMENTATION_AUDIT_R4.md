# Implementation Audit R4: Proposal 077 - Bounded Implementation Closeout Readiness Gates

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/077-bounded-implementation-closeout-readiness-gates.md` |
| Audit report | `docs/proposals/077-bounded-implementation-closeout-readiness-gates_IMPLEMENTATION_AUDIT_R4.md` |
| Generated at | 2026-05-06T09:12:56Z |
| Audit skill | `proposal-implementation-audit` |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree on `main` |
| Audited HEAD | `6188a1e6163cd3c96ee3823ba4f0cd7049f47cd3` |
| Compare base | Implicit current worktree; no PR/range target supplied |
| Worktree status before report | Clean, `main...origin/main [ahead 17]` |
| Proposal state | Active for this audit: checked-in R14 proposal |
| Overall Conformance | **Partial** |
| Overall Implementation Readiness | **Not Ready** |
| Reviewer Selection Reuse | **Not reused** |
| Audit confidence | High for Rust/API/data paths; medium for macOS runtime/accessibility behavior |

## Implementation Target And Compare Base

The user supplied only the proposal path, so this audit evaluates the current worktree. HEAD advanced since R3 from `b2d4d31fc194df81b501062ae2b2ea1ca7349f65` to `6188a1e6163cd3c96ee3823ba4f0cd7049f47cd3` (`Complete P077 audit readiness fixes`).

This audit is read-only except for this R4 report. Existing R1/R2/R3 implementation audits were not used as proposal-review reviewer-selection inputs.

## Prior Proposal-Review Reuse

Reviewer selection was **not reused**.

`discover_prior_review.py` returned no prior proposal-review artifacts for proposal 077. Existing `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection, per the audit skill.

## Selected Reviewers

| Reviewer | Reason selected |
|---|---|
| `chainworks_execution_truth_reviewer` | P077 defines the active state-9 manual-release authority and SQLite truth contract. |
| `rust_reliability_reviewer` | Fingerprint freshness, managed executor deadlines, transaction ordering, and fail-closed behavior are reliability-sensitive. |
| `api_contract_reviewer` | GraphQL, MCP, run-state, exported projection, and macOS readback parity are explicit acceptance items. |
| `observability_rollout_reviewer` | Proposal includes rollout metrics, dependency evidence, release-owner decisions, and rollback. |
| `macos_ui_reviewer` | Proposal mandates macOS read-only Summary/compact/diagnostic/recovery/accessibility behavior. |

## Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| `rust_arch_reviewer` | Rust design issues are narrower than the active-truth and reliability review already selected. |
| `apple_arch_reviewer` | Swift changes are readback/presentation scoped rather than broad app state architecture. |
| `apple_ux_reviewer` | UX concerns are covered through the macOS UI reviewer because the remaining gaps are concrete proposal UI/accessibility requirements. |
| `product_reviewer` | Product metrics are present, but the hard cap is reached and rollout/observability covers metric and decision quality. |
| `rust_security_reviewer` | Governed command authorization is relevant, but no new dominant security surface appeared beyond the selected command/reliability/API lenses. |

## Proposal State And Contract Summary

Proposal 077 introduces `implementation_closeout_readiness_v1` as the only enforcement-mode state-9 manual-release authority. It requires proposal gate proof, current audit truth, controlled evidence, current fingerprints, typed accepted risk lineage, bounded code-refine routing, non-code handoff routing, GraphQL/MCP/run-state/macOS parity, read-only macOS operator surfaces, and rollout evidence before enforcement expansion.

Platform and product scope:

| Surface | Scope |
|---|---|
| Apple | macOS read-only operator UI; no iOS scope |
| Backend/service | Rust control-plane engine, DB, command handler, workflow transition guard |
| API/data | GraphQL, MCP, SQLite migrations, run-state/exported projections |
| Rollout | Advisory/enforcement mode, dependency evidence, metrics, rollback, release-owner decisions |

Leading metric: `false_ready_prevented`.

Guardrail metric: `false_blocks` and `post_release_closeout_gap_reversals`.

Decision checkpoint: Phase 2 enforcement after dependency evidence, parity evidence, current UI evidence, fingerprint p95 threshold, rollback plan, and first cohort review. Current reference evidence says advisory implementation cut only.

## Primary Implementation Flows

1. State 9 synthesizes active proposal gate and closeout readiness truth from SQLite before evaluating manual-release transitions.
2. Operators settle the proposal gate through one governed execute/import/waive command path.
3. GraphQL, MCP, run-state/exported projections, and macOS readback expose the same active closeout readiness summary.
4. Known risks release only through typed accepted lineage or governed settlement.
5. Advisory-to-enforcement rollout is governed by dependency rows, metric ledger, decision snapshot, and rollback rules.

## Fidelity Inventory

### Matches

- Live worktree fingerprinting now resolves `git rev-parse HEAD`, status, and binary diff digest with a 5-second timeout budget.
- Fingerprint unavailable/latency-exceeded now fails closed before manual release.
- Managed gate execute now supports a bounded timeout input/default and tests success, nonzero exit, timeout, and missing script.
- State-9 persistence now uses `execute_closeout_transaction_with_projection_rebuild`, and orchestrator calls it before transition evaluation.
- Active audit status and accepted risk lineage flow into `CloseoutReadinessSummaryAccessor`.
- Run-state projection now includes active `proposal_gate_result_v1` and `implementation_closeout_readiness_v1` rows from `closeout_gate_generations`.
- GraphQL and MCP parity tests are now part of `./scripts/test-gate.sh proposal-077`.
- macOS readback now includes a diagnostics sheet, secondary blocker rows, recovery/backlink labels, compact signal label, generation copy, and accessibility labels.
- New `docs/reference/p077-closeout-readiness-ui-evidence.md` supplies token/accessibility evidence and is checked by the P077 gate.
- New rollout evidence marks dependency rows passed for the advisory implementation cut and records that enforcement does not expand from the document alone.

### Divergences

- Managed executor receipts no longer digest actual stdout/stderr content: the process redirects both streams to null and hashes empty byte slices.
- macOS compact activation, focus return, copy-failure fallback, VoiceOver throttling, and backlink routing are represented mostly as labels/fixture strings, not proven runtime behavior.
- UI contrast evidence is a semantic-color spot check, not measured contrast evidence for the proposal's named surfaces and accessibility modes.
- Rollout evidence is still a reference/decision record; no live cohort metric rows, decision payload persistence, or rollback execution path was found.
- The canonical P077 gate still documents exclusions for live state-9 orchestrator integration, Swift workspace tests, macOS UI runtime, and VoiceOver fixtures.

### Ambiguities / Evidence Gaps

- No live orchestrator state-9 run was executed against SQLite during this audit.
- Swift tests and UI runtime screenshots were not executed; repository policy keeps local UI smoke tests out of scope unless explicitly requested.
- Example workflows contain P077 transition guards but do not declare `closeout_readiness_mode`; current code supports metadata and command fallback, with absent mode defaulting advisory.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 5 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed Requirement Audit

### REQ-001 - Active Contract IDs, Statuses, And Decisions

- Source: Proposal lines 41-93.
- Status: **Implemented**.
- Evidence: `code`, `migration`, `tests-run`.
- Mapping: domain contracts, closeout readiness types, migrations, active generation tables, and proof-gate tests cover the contract IDs, status values, and decisions.
- Note: The P077 canonical gate passed on audited HEAD.

### REQ-002 - Decision Matrix And Gate-Cause Routing

- Source: Proposal lines 94-127 and 605-613.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: `control-plane/crates/engine/tests/p077_proof_gate.rs` covers missing gates, failed gates, code blockers with budget, non-code handoff, accepted risks, green manual release, soft convergence, and stale exported JSON exclusion.

### REQ-003 - Current Fingerprint And Latency Fail-Closed Rule

- Source: Proposal lines 103-119 and 494-500.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: `control-plane/crates/engine/src/closeout_fingerprint.rs:74` resolves live worktree truth with timeout; `closeout_fingerprint.rs:116` reads HEAD; `closeout_fingerprint.rs:117` and `:122` hash dirty/diff state. `control-plane/crates/engine/src/orchestrator.rs:1539` feeds that truth into synthesis. `control-plane/crates/engine/src/synthesizers/closeout_readiness.rs:107` fails closed on latency/unavailable truth.

### REQ-004 - Governed Gate-Settlement Command

- Source: Proposal lines 129-149 and 595-602.
- Status: **Implemented**.
- Evidence: `code`, `tests-found`, `tests-run`.
- Mapping: `control-plane/crates/domain/src/commands.rs:340` defines the action-enum settlement command and lineage fields; `control-plane/crates/engine/src/command_handler.rs:3321` binds caller principal and validates authorization/accepted risk lineage before settlement.

### REQ-005 - P077 ProposalGateExecutor

- Source: Proposal lines 161-180.
- Status: **Partially Implemented**.
- Evidence: `code`, `tests-found`, `tests-run`.
- Mapping: `control-plane/crates/engine/src/command_handler.rs:241` executes `scripts/test-gate.sh proposal-077`; `domain/src/commands.rs:357` adds timeout input; `command_handler.rs:347` enforces timeout and kill; tests at `command_handler.rs:4777`, `:4805`, `:4835`, and `:4861` cover pass/fail/timeout/missing script.
- Gap: Receipt `stdout_digest` and `stderr_digest` are not computed from actual process output. The executor redirects both streams to null (`command_handler.rs:259`) and hashes empty slices (`command_handler.rs:280`). The proposal requires stdout/stderr digests as gate outputs.

### REQ-006 - Readiness Mode Storage And Accessor

- Source: Proposal lines 182-196.
- Status: **Implemented**.
- Evidence: `migration`, `code`, `tests-run`.
- Mapping: DB migration adds frozen run-owned mode and overrides; workflow compiler extracts `workflow.closeout_readiness_mode`; run creation persists plan mode or command fallback; accessor fallback defaults absent legacy mode to advisory.

### REQ-007 - State-9 Closeout Transaction Helper

- Source: Proposal lines 216-227.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: `control-plane/crates/db/src/repos/closeout.rs:64` activates gate/readiness together; `closeout.rs:158` wraps activation with projection rebuild before returning success; `control-plane/crates/engine/src/orchestrator.rs:1588` uses the rebuild helper before transition evaluation. DB tests at `closeout.rs:1222` prove projection parity after closeout transaction.

### REQ-008 - Transition Guard Reads Active SQLite Truth

- Source: Proposal lines 41-42, 212-223, and 613.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: `orchestrator.rs:708` and `:741` synthesize closeout readiness before transition evaluation; `orchestrator.rs:5849` detects states that reference `implementation_closeout_readiness_v1`; proof-gate tests cover active truth over stale exported JSON.
- Note: Live state-9 orchestrator integration remains outside the current gate, but the implementation path and focused tests support the requirement.

### REQ-009 - Controlled Evidence And Active Audit Truth

- Source: Proposal lines 24-30, 94-101, and 611.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: orchestrator sources controlled report truth from active artifact contracts; closeout summary reads active `audit_report_v1`; DB tests cover audit-status readback and controlled report missing/fail behavior.

### REQ-010 - Typed Risk Acceptance Lineage

- Source: Proposal lines 198-214 and 610.
- Status: **Implemented**.
- Evidence: `code`, `migration`, `tests-run`.
- Mapping: `domain/src/risk_lineage.rs` defines typed sources and required fields; synthesizer rejects free-form risk text; DB persists `accepted_risks_json`; tests cover accepted lineage readback and ready-with-risks gating.

### REQ-011 - GraphQL, MCP, Run-State, And Exported Projection Parity

- Source: Proposal lines 156-157, 214, and 614.
- Status: **Implemented**.
- Evidence: `code`, `tests-run`.
- Mapping: GraphQL exposes documented and compatibility fields; MCP runs/reports expose both names; run-state projection includes P077 closeout rows from `closeout_gate_generations`; P077 gate now runs GraphQL parity, MCP runs.get/list/report parity, and DB projection parity tests.

### REQ-012 - macOS Read-Only UI Surface

- Source: Proposal lines 229-245, 247-267, and 615.
- Status: **Partially Implemented**.
- Evidence: `code`, `tests-found`, `config`.
- Mapping: Swift read boundary decodes closeout readiness; presenter builds state labels, diagnostics rows, secondary blocker rows, recovery text, backlink label, compact label, and copy metadata; `RunsHomeView` renders a Closeout Readiness card and diagnostics sheet.
- Gap: Compact activation does not implement expand/scroll/focus behavior; backlink is a label rather than route behavior; recovery lifecycle is text, not acknowledgement/correlation/stalled row state; secondary blocker keyboard ordering is not proven through UI/runtime tests.

### REQ-013 - Accessibility, Focus, Copy, Generation Fixtures

- Source: Proposal lines 236-245, 257-267, 270-359, and 616.
- Status: **Partially Implemented**.
- Evidence: `code`, `tests-found`, `config`.
- Mapping: Tests cover status labels, generation copy labels, diagnostics labels, compact accessibility labels, diagnostic rows, recovery text, and backlink labels.
- Gap: No bounded VoiceOver announcement throttling implementation/fixture, no copy-failure fallback flow, no actual focus-return test, no keyboard traversal proof for secondary blockers, and no re-openable explainer behavior was found. `docs/reference/test-gates.md:1033` explicitly excludes macOS UI/accessibility and VoiceOver fixtures from the P077 gate.

### REQ-014 - Token Mapping And Contrast Evidence

- Source: Proposal lines 360-408, 576, and 595-604.
- Status: **Partially Implemented**.
- Evidence: `design-reference`, `config`, `tests-run`.
- Mapping: `docs/reference/p077-closeout-readiness-ui-evidence.md:11` adds a token mapping table and `:29` adds contrast evidence. `scripts/test-gate.sh:450` requires this evidence file and fields.
- Gap: The evidence is a semantic-color spot check, not measured contrast results for `readyWithRisks`/amber fallbacks on `cardElevated` and `compactCapsule` in standard, High Contrast, Reduce Transparency, and Differentiate Without Color modes.

### REQ-015 - Rollout Metrics, Dependency Evidence, Decision Payload, And Rollback

- Source: Proposal lines 410-579.
- Status: **Partially Implemented**.
- Evidence: `design-reference`, `config`, `tests-run`.
- Mapping: `docs/reference/p077-rollout-dependency-evidence.md:15` marks dependency rows passed for the advisory implementation cut; `:31` defines the metric ledger; `:52` records a current advisory decision snapshot; `:63` defines rollback rules. The P077 gate checks required fields.
- Gap: The document itself says enforcement expansion remains advisory until live cohort evidence exists. No live metric event source, cohort row persistence, release-owner decision payload storage, or executable rollback/advisory reversion path was found.

### REQ-016 - Canonical P077 Proof Gate Registration

- Source: Proposal lines 605-617.
- Status: **Implemented**.
- Evidence: `config`, `tests-run`.
- Mapping: `scripts/test-gate.sh:5446` registers `proposal-077|p077`, validates rollout/UI evidence files, and runs Rust domain/db/engine, GraphQL, MCP, and proof-gate tests. `docs/reference/test-gates.md:1014` documents scope and exclusions.

## Reviewer / Lens Scorecard

| Lens | Reviewer | Result | Top risk | Confidence |
|---|---|---|---|---|
| Proposal conformance | Track 1 | Partial | Five explicit requirements remain partially implemented | High |
| Active execution truth | `chainworks_execution_truth_reviewer` | Mostly passes | Live state-9 graph is not gate-proven, but code path is present | Medium |
| Rust reliability | `rust_reliability_reviewer` | Partial | Managed receipt digests do not reflect actual stdout/stderr output | High |
| API contract | `api_contract_reviewer` | Passes | GraphQL/MCP/run-state projection parity is now gate-backed | High |
| Observability/rollout | `observability_rollout_reviewer` | Partial | Static advisory evidence exists, but no live cohort/rollback machinery | High |
| macOS UI | `macos_ui_reviewer` | Partial | Interaction/accessibility behavior is mostly readback text/labels, not runtime-proven behavior | Medium |
| Readiness | Track 2 | Not Ready | Major proposal-critical gaps remain despite passing P077 gate | High |

## Routed Specialist Findings

### READY-001 - Managed Executor Does Not Digest Actual Gate Output

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-005
- Evidence: `code`, `tests-run`
- Evidence references: `control-plane/crates/engine/src/command_handler.rs:259`, `control-plane/crates/engine/src/command_handler.rs:280`, `control-plane/crates/engine/src/command_handler.rs:4798`
- Why it matters: Proposal 077 requires stdout/stderr digests as executor outputs. Hashing empty streams for every execution loses proof that the receipt represents the gate's actual emitted evidence.
- Recommended action: Capture stdout/stderr with bounded buffers or spool files, compute digests from the actual byte streams, and retain only digests/truncated diagnostics as needed.
- Acceptance criteria: Tests assert stdout/stderr digest values change when the script output changes and still remain bounded for large output.

### UI-001 - macOS Interaction And Accessibility Are Still Label-Level

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-012, REQ-013
- Evidence: `code`, `tests-found`, `config`
- Evidence references: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:3780`, `Chainworks Forge/Views/RunsHomeView.swift:1041`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:640`, `docs/reference/test-gates.md:1033`
- Why it matters: The proposal requires compact activation, focus return, bounded announcements, copy fallback, keyboard secondary blockers, backlink routing, and re-openable explainer behavior. The implementation exposes much of this as labels/rows, but not as proven interaction behavior.
- Recommended action: Add runtime-backed Swift/UI fixtures for compact activation, focus return, copy success/failure/fallback, secondary blocker keyboard order, Diagnostics/Artifacts backlink routing, and VoiceOver announcement throttling.
- Acceptance criteria: The P077 gate or a documented companion gate runs these fixtures, or the audit records explicit manual UI evidence for each behavior.

### UI-002 - Contrast Evidence Is Not Measured Against The Proposal's Named Modes

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-014
- Evidence: `design-reference`, `config`
- Evidence references: `docs/reference/p077-closeout-readiness-ui-evidence.md:29`, `docs/proposals/077-bounded-implementation-closeout-readiness-gates.md:360`
- Why it matters: The proposal made Phase 0 contrast measurement a cutover gate for ready-with-risks and amber fallback candidates on specific surfaces and accessibility modes. The new evidence relies on semantic-color reasoning instead of measured results.
- Recommended action: Record measured contrast results or approved design-system evidence for standard, High Contrast, Reduce Transparency, and Differentiate Without Color modes on the actual card/compact surfaces.
- Acceptance criteria: The UI evidence table includes measured or cited numeric/official proof for each required surface/mode pair and the gate verifies those entries.

### OPS-001 - Rollout Evidence Still Does Not Operate The Rollout

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-015
- Evidence: `design-reference`, `config`
- Evidence references: `docs/reference/p077-rollout-dependency-evidence.md:43`, `docs/reference/p077-rollout-dependency-evidence.md:52`, `scripts/test-gate.sh:423`
- Why it matters: Enforcement expansion depends on real cohort metrics, release-owner decision payloads, waiver lineage, and rollback. The current artifact is a useful static advisory record, but it does not persist or execute those rollout decisions.
- Recommended action: Add durable cohort metric rows, release-owner decision records, rollback/advisory migration records, and a gate or command proving false-block/closeout-gap rollback behavior.
- Acceptance criteria: A test or operator command records a go/no-go decision from metric rows and reverts new eligible runs to advisory when a rollback trigger is recorded.

### READY-002 - Canonical Gate Still Excludes End-To-End UI And Live State-9 Proof

- Reviewer: `chainworks_execution_truth_reviewer`, `macos_ui_reviewer`
- Severity: Major
- Confidence: Medium
- Related requirements: REQ-008, REQ-012, REQ-013, REQ-016
- Evidence: `config`, `tests-run`
- Evidence references: `docs/reference/test-gates.md:1033`, `scripts/test-gate.sh:5446`
- Why it matters: Passing `proposal-077` is now strong backend/API evidence, but the documented exclusions are exactly the remaining operator-facing and live orchestration risks.
- Recommended action: Add a companion integration gate for live state-9 SQLite transition behavior and a Swift/UI accessibility gate, or explicitly document manual evidence accepted for enforcement cutover.
- Acceptance criteria: Same-HEAD verification covers live state-9 transition evaluation plus macOS UI/accessibility behavior, or the proposal closeout records a governed waiver for those excluded proofs.

## Readiness Checklist

| Item | Status | Evidence |
|---|---|---|
| Build or canonical gate status | Passed | `./scripts/test-gate.sh proposal-077` passed on `6188a1e6163cd3c96ee3823ba4f0cd7049f47cd3`. |
| Core state-9 flow integration | Partial | Focused Rust and DB tests passed; no live orchestrator state-9 SQLite run executed. |
| GraphQL/MCP/API parity | Passed | P077 gate ran GraphQL parity, MCP runs.get/list parity, and report readback parity tests. |
| Run-state/exported projection parity | Passed for focused DB proof | DB projection parity test passed. |
| macOS UI states | Partial | Presenter fixtures found; Swift tests not run and runtime UI not exercised. |
| Accessibility/focus/copy | Partial | Labels and fixture strings exist; VoiceOver/focus/copy-fallback runtime behavior not proven. |
| Token/contrast evidence | Partial | Static token/semantic contrast evidence exists; measured required-mode evidence absent. |
| Rollout/rollback readiness | Partial | Dependency rows passed for advisory cut; live metrics, decisions, and rollback execution absent. |
| Full regression or canonical gate | Canonical proposal gate passed | Full repository gate was not run; a successful readiness verdict is not claimed. |

## Verification Log

| Command / Check | Result |
|---|---|
| `date -u '+%Y-%m-%dT%H:%M:%SZ'` | `2026-05-06T09:12:56Z`. |
| `git rev-parse HEAD` | `6188a1e6163cd3c96ee3823ba4f0cd7049f47cd3`. |
| `git status --short --branch` | Clean worktree, `main...origin/main [ahead 17]`, before creating R4. |
| `report_path.py ...077...md` | Returned `docs/proposals/077-bounded-implementation-closeout-readiness-gates_IMPLEMENTATION_AUDIT_R4.md`. |
| `discover_prior_review.py ...077...md` | Returned no prior proposal-review artifacts. |
| `git diff --stat b2d4d31f..HEAD` | Confirmed P077 fixes changed fingerprint, executor, DB closeout, GraphQL/MCP, Swift UI, UI evidence, rollout evidence, and test gate files. |
| `./scripts/test-gate.sh proposal-077` | Passed. Included DB closeout tests (7), GraphQL parity (1), MCP parity (2), and P077 proof gate (10), with warnings only. |
| Focused source reads | Inspected fingerprint resolver, orchestrator, command handler, DB closeout/projection code, GraphQL/MCP readbacks, Swift presenter/view, Swift tests, UI evidence, rollout evidence, and gate docs. |

## Final Verdict

Overall Conformance: **Partial**.

Overall Implementation Readiness: **Not Ready**.

R4 is a major improvement over R3. The backend/API proposal spine is now largely implemented and the canonical P077 gate passes on the audited HEAD. No in-scope requirement remains fully missing. The implementation is still not proposal-complete because executor output digests, macOS interaction/accessibility proof, measured contrast evidence, and live rollout/rollback mechanics remain partial.

## Recommended Next Actions

1. Compute stdout/stderr digests from actual managed executor output with bounded capture.
2. Add Swift/UI fixtures or manual evidence for compact activation, focus return, copy fallback, secondary blocker keyboard access, backlink routing, re-openable explainer, and bounded VoiceOver announcements.
3. Replace semantic contrast assertions with measured or officially cited evidence for the required surfaces and accessibility modes.
4. Persist live cohort metrics, release-owner decision payloads, waiver lineage, and rollback/advisory migration records.
5. Add a companion gate for live state-9 SQLite transition behavior and macOS UI/accessibility evidence before enforcement cutover.
