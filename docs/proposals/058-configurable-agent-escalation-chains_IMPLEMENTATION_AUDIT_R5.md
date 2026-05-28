# P058 Implementation Audit R5: Configurable Agent Escalation Chains

## Metadata

- **Audit type:** Proposal implementation audit (`proposal-implementation-audit`)
- **Proposal:** `docs/proposals/058-configurable-agent-escalation-chains.md`
- **Proposal revision:** `p058-r14-2026-05-07`
- **Proposal state:** Active implementation-closeout proposal; not retired
- **Repository:** `/Users/user/Documents/Chainworks Forge`
- **Implementation target:** Current worktree, branch `main`
- **Target HEAD:** `0559ff1afa298c4ce34512368c16b194c47ec8a5` (`Close P058 audit gaps`)
- **Compare base:** implicit current `origin/main`; merge-base equals target HEAD
- **Delta inspected:** `d67161ef91417552b2494e402e8b5d4d51a99e8f..HEAD` for changes since the prior audited tree
- **Audit timestamp:** `2026-05-28T17:11:11Z`
- **Worktree status before report write:** clean
- **Report path:** `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R5.md`

## Verdict

- **Overall Conformance:** **Partial**
- **Overall Implementation Readiness:** **Not Ready for full P058 / broad release**
- **Implementation-closeout slice:** Rust/control-plane and governed readback slice are **ready with risks** on same-tree `proposal-058` gate evidence.
- **Reviewer Selection Reuse:** **Not reused**. No prior proposal-review artifacts were discovered by the helper.
- **Audit Confidence:** High for Rust/control-plane, API/readback, migrations, metrics, and gate status. Medium for macOS UI because the remaining gaps are mostly proven by code absence and lack of runtime visual/accessibility evidence.

The R5 target materially improves the previous state: the P058 metric inventory test now exists and is invoked by the gate, reference migration numbers are synchronized to `076/077/078`, Swift actor isolation was addressed by making `EscalationSnapshot.build` nonisolated, and the canonical `./scripts/test-gate.sh proposal-058` gate passed on the audited HEAD. The remaining blocker is narrower: the full detailed governed macOS UI contract in the proposal is still only partially implemented/proven.

## Prior Review Reuse

Discovery command:

```bash
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/058-configurable-agent-escalation-chains.md
```

Result:

```json
{"artifacts":[]}
```

- **Reuse classification:** Not reused
- **Reason:** No `<proposal>.review/`, sibling proposal-review report, evidence pack, research pack, or matching repo-local review artifact was found.
- **Implementation audits ignored for reviewer selection:** Existing `*_IMPLEMENTATION_AUDIT_R*.md` files were not used for reviewer routing.

## Selected Reviewers

- `rust_arch_reviewer`: Rust crate boundaries, RunPlan/policy ownership, persistence model.
- `rust_reliability_reviewer`: claim/start idempotency, recovery, retry-after, force-detach, capacity and deadline behavior.
- `api_contract_reviewer`: GraphQL/MCP/report readback shape, redaction, forward-compatible raw-string vocabulary.
- `observability_rollout_reviewer`: metrics inventory/emission, rollout contract fixture, broad-release evidence.
- `macos_ui_reviewer`: governed macOS read surface, notification/menu-bar attention, accessibility and layout commitments.

Rejected close alternatives:

- `apple_ux_reviewer`: covered under `macos_ui_reviewer` to stay within the five-reviewer cap; no separate user-research change was introduced.
- `security_reviewer`: relevant security/redaction behavior is covered by P058 schema/readback tests and API contract checks; no new auth boundary was introduced in the delta.
- `performance_reviewer`: long-running threshold trends are deferred release evidence, not a new hot-path benchmark claim in this diff.

## Proposal Contract Summary

P058 commits to a repo-owned `escalation_policy_v1` system with these in-scope surfaces:

- Rust control plane authority for policy resolution, trigger classification, tier advancement, retry budgets, pause/resume legality, kill switch, persistence, recovery, and readback.
- Ordered tiers: `same_backend_retry`, `backend_profile`, `lead_mediation`, `pause`.
- Typed trigger and pause vocabulary, forward-compatible raw-string readback, and stable operator action hints/runbook anchors.
- Frozen policy hash, binding data, tier order, trigger vocabulary, and rollout override state in compiled `RunPlan` truth.
- Durable SQLite ledger, execution metadata, runtime facts, and event journal with idempotency and redaction-version enforcement.
- GraphQL, MCP, report, and macOS readback parity with redaction/caps for non-operator callers.
- Rollout contract, metrics inventory/emission, kill-switch and data-preserving rollback posture.
- Governed macOS read-only UI: `EscalationReadAdapter`, status capsule, banner stack, lineage, pause card, trace timeline, drift review sheet, command mirrors, MenuBarExtra/read attention, dock badge, keyboard/accessibility behavior, read-pipeline states, and layout/density rules.

## Platform And Scope

- **Apple platform:** macOS SwiftUI presentation only; governed macOS is read/subscription presentation, not lifecycle authority.
- **Backend/service:** Rust control-plane engine, workflow compiler, db repos/migrations, GraphQL server, MCP server.
- **Data:** SQLite migrations and repositories for ledger, execution metadata, events, runtime facts, metrics.
- **API:** GraphQL `runEscalationReadback`, MCP `runs.get`, report/readback fixtures.
- **Operations:** rollout contract fixture, metrics inventory, release-closeout follow-ups.

## Primary Flows Audited

1. **Policy compile/freeze:** YAML policy parses strictly, resolves backend profiles, rejects unsafe/ambiguous bindings, freezes hash/binding data into `RunPlan`.
2. **Claim/start execution:** `InvokeAgent` claim/start pre-creates execution identity, ledger row, source-generation claim, and execution metadata in the scheduler-owned path.
3. **Failure settlement and tier selection:** runtime facts classify failures, write shadow/selection fields, advance same-backend/backend-profile/lead-mediation tiers, or pause fail-closed.
4. **Readback and redaction:** GraphQL/MCP/report expose raw-string escalation readback, caps, redacted event data, and non-operator summary behavior.
5. **Operator presentation and rollout:** macOS renders read-only escalation state, dock/menu/attention surfaces signal paused work, and rollout metrics/evidence support release decisions.

## Implementation Delta Since Prior Audited Tree

Changed implementation/doc files in `d67161e..0559ff1a`:

- `Chainworks Forge/Engine/NotificationService.swift`
- `Chainworks Forge/Models/EscalationState.swift`
- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `control-plane/crates/db/src/metrics.rs`
- `docs/reference/escalation-policies.md`
- `docs/reference/rust-control-plane.md`
- `docs/reference/test-gates.md`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R4.md`

Notable improvements:

- P058 metric inventory is now declared as `P058_REQUIRED_METRICS` and the focused unit test `proposal_058_required_metric_names_are_declared` exists.
- `scripts/test-gate.sh proposal-058` invokes the P058 metric inventory test.
- Reference docs now name current migrations `076_p058_escalation_schema.sql`, `077_p058_escalation_redaction_version.sql`, and `078_p058_escalation_idempotency.sql`.
- Swift P058 tests now cover component construction, screen-state matrix, read-pipeline states, dock/menu count aggregation, P031 readback mapping, and pasteboard copy.
- `NotificationService` now aggregates P058 escalation attention into the dock badge and menu-bar enabled state.

## Fidelity / Divergence / Gaps

Matches:

- Policy schema, strict validation, frozen policy hash, and tier vocabulary are covered by workflow and engine/db tests.
- Durable ledger, metadata, event journal, redaction version, idempotency, JSON validation, and runtime-facts/shadow fields are covered by Rust tests.
- Scheduler behavior covers same-backend retry, backend-profile/lead-mediation follow-up, pause tier suppression of legacy retry, kill switch, chain deadline, capacity probe threshold, provider force-detach, launch storm, late-frame journaling, and startup replay.
- GraphQL/MCP readback parity, non-operator redaction, event caps, and P031 Swift readback mapping have focused tests.
- Rollout-contract readback fixture exists and now points to concrete release-closeout follow-up evidence.
- Same-tree `proposal-058` gate passed on the audited HEAD.

Divergences:

- The macOS read surface remains a shallow component set compared with the detailed proposal contract. Examples: the status capsule renders a state label rather than the proposed state/tier/trigger field order; lineage does not implement Tier 0 baseline, retry collapse, row disclosure, fixed columns, narrow-row fallback, or shadow-row styling; command rows do not enforce the disabled reason/truncation/mirror contract; MenuBarExtra is represented by a list view and service flag but no actual `MenuBarExtra` call site was found.
- P058 human attention semantics are incomplete: `NotificationService.applyP058EscalationSnapshots` updates counts and menu-bar state, but the P058 path does not call `NSApp.requestUserAttention(.informationalRequest)` or hold/cancel a P058 attention token as specified.
- Full Keyboard Access tab-order, focus movement/return, contrast, reduced motion, VoiceOver order, narrow layout breakpoints, and remote visual/runtime proof are not implemented or executed by the focused tests.

Ambiguities / evidence gaps:

- The proposal and rollout fixture classify remote visual soak, long-run metric thresholds, and operational drills as release-closeout evidence rather than missing implementation paths. That is concrete enough to avoid treating those items as backend code gaps, but they remain broad-release blockers.
- No runtime screenshot, UI automation, or remote visual/accessibility run was produced for this audit.

## Residual Scope / Follow-up Ownership

| Residual item | Owner evidence | Blocks conformance/readiness? | Notes |
|---|---|---:|---|
| Detailed governed macOS UI contract: capsule field order/collapse, lineage layout/collapse/disclosure/shadow rows, command truncation/disabled reason, MenuBarExtra compact item, keyboard/focus/contrast/reduced motion behavior | No concrete follow-up proposal found | Yes | Explicit proposal commitments remain in `Ux Ui Notes`; current code/tests cover only a subset. |
| P058 human attention request and cancellation-token lifecycle | No concrete follow-up proposal found | Yes | Dock/menu aggregation exists, but the informational attention request path is absent for P058 snapshots. |
| Remote macOS visual/runtime/accessibility soak | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | Blocks broad release, not backend implementation closeout | Follow-up is explicit evidence ownership, not code ownership. |
| Long-running metric thresholds/trends | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | Blocks broad release | Metric inventory/emission is implemented; threshold trend evidence is not collected. |
| Live operational drills: SIGTERM/restart, populated migration drill, release receipt pack | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | Blocks broad release | Focused fake/fixture coverage exists; live release evidence remains outstanding. |

## Requirement Audit

| ID | Requirement | Proposal source | Status | Evidence | Notes |
|---|---|---|---|---|---|
| REQ-001 | Define `escalation_policy_v1` in repo-owned catalog/workflow data with backend profile IDs and ordered tier kinds. | Goals, Policy Schema | Implemented | `workflow` policy schema tests, `docs/reference/escalation-policies.md` | Covers strict parse and all four tier kinds. |
| REQ-002 | Support typed triggers and pause reason vocabulary with forward-compatible raw string handling. | Goals, Pause Reason Catalog | Implemented | Swift pause/tier coverage tests, Rust schema tests | 13 pause reasons and unknown raw round-trip are tested. |
| REQ-003 | Freeze policy hash, binding data, tier order, trigger vocabulary, and rollout override state into compiled truth. | Goals, Architecture | Implemented | Engine/workflow tests, reference docs | Policy resolution uses frozen `RunPlan`; no live YAML fallback evidence found in audited path. |
| REQ-004 | Persist ledger, execution metadata, runtime facts, and events with stable idempotency and no overlapping active tier. | Goals, Persistence | Implemented | DB migrations `076/077/078`, db/engine tests | Idempotency and redaction-version enforcement are tested. |
| REQ-005 | Scheduler-owned claim/start and tier selection must use durable execution identity and support same-backend, backend-profile, lead-mediation, and pause behavior. | Implementation Sync, Architecture, Runtime | Implemented | `proposal-058` gate, engine claim-start and scheduler tests | Backend/control-plane slice is strong. |
| REQ-006 | Runtime controls must fail closed for kill switch, deadlines, retry-after/capacity, force-detach, launch storm, late frames, and recovery inconsistency. | Defaults, Provider Classifier, Recovery | Implemented | Engine/db tests in `proposal-058` gate | Fake/fixture coverage, not live provider soak. |
| REQ-007 | GraphQL, MCP, report, and macOS readback expose parity shape, redaction, caps, and forward-compatible raw values. | Goals, Rollout Contract, Readback Lanes | Implemented | GraphQL/MCP tests, P031 Swift tests, rollout fixture | Non-operator MCP summary excludes sensitive fields. |
| REQ-008 | Metrics inventory and production emission from durable escalation state are declared and gate-enforced. | Rollout Contract Metrics, Metrics Emission | Implemented | `control-plane/crates/db/src/metrics.rs`, `scripts/test-gate.sh`, `cargo test -p db proposal_058_required_metric_names_are_declared` | R5 resolves the prior zero-test gate gap. |
| REQ-009 | Governed macOS must remain read-only and source presentation from `EscalationReadAdapter`/GraphQL DTOs rather than local authority. | Ux Ui Notes, Authority Boundary | Implemented | `EscalationReadAdapter`, P058/P031 Swift tests | No mutation path was found in the audited read components. |
| REQ-010 | Governed macOS detailed component, accessibility, notification, dock/menu, and layout contracts are implemented and proven. | Ux Ui Notes lines 65-247, Notifications | Partially Implemented | `EscalationReadSurfaceViews.swift`, `NotificationService.swift`, P058 Swift tests, targeted `rg` | Components exist and compile, but many required detailed states/behaviors are absent or unproven. See `UI-001`. |
| REQ-011 | Rollout contract/readback fixture and release evidence ownership exist. | Rollout Contract V1, Rollout Plan | Partially Implemented | `p058-full-surface.fixture.json`, `release-closeout-followups.json` | Implementation-closeout fixture exists; broad-release evidence remains explicitly open. |
| REQ-012 | Reference docs and canonical gate must describe the implemented system accurately. | Implementation Sync, Gate Aliases | Implemented | `docs/reference/escalation-policies.md`, `docs/reference/rust-control-plane.md`, `docs/reference/test-gates.md`, passing gate | Migration numbers and metric-gate scope are now synchronized. |

## Reviewer Findings

### UI-001 [Major] Detailed governed macOS UI contract remains partial

The proposal pins implementation-grade macOS UI behavior for status capsule field order/collapse, banner cooccurrence, lineage columns/retry collapse/disclosure/shadow rows, pause-card countdown/responsive bounds, command disabled reasons/truncation/mirrors, MenuBarExtra badge/overflow/sorting, focus order, contrast, reduced motion, and VoiceOver ordering. The implementation provides constructible SwiftUI components, but inspection shows a much smaller surface: `EscalationStatusCapsule` renders one state label; `EscalationLineageView` is a simple row list; `EscalationMenuBarList` exists but no actual `MenuBarExtra` call site was found; `NotificationService.applyP058EscalationSnapshots` updates counts only; targeted searches found no P058 full-keyboard, retry-collapse, shadow-row, overflow, or narrow-layout implementation. The Swift gate proves component construction and a few state helpers, not the full proposal UI contract.

Impact: REQ-010 remains partially implemented, so full proposal conformance cannot roll up to Implemented even though backend/readback gates pass.

### UI-002 [Major] P058 human attention behavior is not wired to informational user-attention requests

The proposal requires paused runs needing operator action to increment Dock badge and call `NSApp.requestUserAttention(.informationalRequest)` when backgrounded, with a MainActor-held cancellation token cancelled on activation or pause clear. The current P058 path aggregates `pendingAttentionCount` and `isMenuBarEnabled`, but the P058 snapshot path does not call `requestUserAttention(.informationalRequest)` and does not implement the cancellation-token lifecycle. The only `requestUserAttention` helper in `NotificationService` is currently used by P081 operator alerts.

Impact: paused escalation attention may be visible in counts but does not satisfy the native attention contract.

### READY-001 [Major] Broad-release evidence remains intentionally outstanding

The rollout fixture and follow-up evidence file explicitly identify remote UI soak, long-running metric thresholds, and operational drills as broad-release blockers. This is acceptable as implementation-closeout evidence ownership, but it means full P058/broad-release readiness is not achieved by this audit.

Impact: backend/control-plane implementation can be closed out with risk tracking, but user-facing release should stay held until these evidence packs are collected.

## Resolved Since The Prior Audited Tree

- **Metrics gate coverage:** `proposal_058_required_metric_names_are_declared` now lists one test and `scripts/test-gate.sh proposal-058` invokes it.
- **Reference migration drift:** docs now refer to `076_p058_escalation_schema.sql`, `077_p058_escalation_redaction_version.sql`, and `078_p058_escalation_idempotency.sql`.
- **Swift isolation warning:** `EscalationSnapshot.build` is now `nonisolated`; the P058 Swift gate did not emit the previous actor-isolation warning.
- **MacOS component presence:** R5 adds governed SwiftUI component construction and focused tests, reducing but not eliminating the UI contract gap.

## Readiness Checklist

- [x] Proposal file exists and was audited at current HEAD.
- [x] Prior proposal-review artifact discovery was performed.
- [x] Current worktree was clean before report write.
- [x] Same-tree canonical `proposal-058` gate passed on audited HEAD.
- [x] Rust schema, idempotency, runtime facts, scheduler, GraphQL, MCP, and metrics inventory have focused passing tests.
- [x] Rollout fixture and release-closeout evidence ownership exist.
- [ ] Full detailed macOS UI contract is implemented/proven.
- [ ] P058 native human-attention request and cancellation-token behavior is implemented/proven.
- [ ] Remote visual/accessibility soak evidence is collected.
- [ ] Long-running metric threshold and operational drill evidence is collected.

## Verification Log

Commands run:

```bash
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/058-configurable-agent-escalation-chains.md
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/058-configurable-agent-escalation-chains.md
git show --stat --oneline --decorate --no-renames d67161ef91417552b2494e402e8b5d4d51a99e8f..HEAD
git diff --name-status d67161ef91417552b2494e402e8b5d4d51a99e8f..HEAD -- docs/proposals/058-configurable-agent-escalation-chains.md docs/reference/escalation-policies.md scripts/test-gate.sh "Chainworks Forge" "Chainworks ForgeTests" control-plane
cd control-plane && cargo test -p db proposal_058_required_metric_names_are_declared --lib -- --list
./scripts/test-gate.sh proposal-058
rg -n "isMenuBarEnabled|MenuBarExtra|applyP058EscalationSnapshots|requestUserAttention\(\.informationalRequest|EscalationMenuBarList" "Chainworks Forge" "Chainworks ForgeTests" -g '*.swift'
rg -n "retry collapse|Retry n|Full Keyboard|tab-order|shadow|50%|dashed|tooltip|overflow|Show all paused|Countdown|Ready to retry|interactiveDismissDisabled|public.json" "Chainworks Forge/Views/EscalationReadSurfaceViews.swift" "Chainworks ForgeTests/Proposal058Tests.swift"
```

Key results:

- `discover_prior_review.py`: no prior proposal-review artifacts.
- P058 metric test list: `metrics::tests::proposal_058_required_metric_names_are_declared: test` and `1 test, 0 benchmarks`.
- `./scripts/test-gate.sh proposal-058`: passed.
  - Swift focused gate: 22 P058 tests passed.
  - Rust domain runtime facts: 3 tests passed.
  - Rust engine focused P058 tests: 14 tests passed.
  - Rust db runtime facts: 10 tests passed.
  - Rust db claim-start tests: 2 tests passed.
  - Rust engine claim-start tests: 21 tests passed.
  - GraphQL runtime/readback tests: 6 tests passed.
  - MCP runtime/readback tests: 4 tests passed plus focused lib readback tests.
  - Escalation schema tests: 60 tests passed.
  - Workflow escalation policy schema tests: 30 tests passed.
  - DB payload JSON shape tests: 25 tests passed.
  - `cargo check -p engine`, `cargo check -p graphql-server`, and `cargo check -p mcp-server` completed.
- Non-fatal existing warnings appeared in Rust/Swift build output; none failed the gate.

## Required Actions

1. Either implement and prove the remaining detailed macOS UI contract in P058, or narrow/retire that contract into a concrete follow-up proposal with explicit ownership.
2. Wire and test P058 native human-attention semantics: actual informational `requestUserAttention`, background/activation handling, cancellation token, and pause-clear behavior.
3. Collect broad-release evidence tracked by `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` before broad user-facing release: remote UI/accessibility soak, metric threshold trends, and operational drills.

## Final Assessment

The backend/control-plane portion of P058 is now well supported by same-tree evidence and can be treated as implementation-closeout ready with tracked risk. The full proposal, however, still contains explicit governed macOS UI and native attention commitments that are only partially implemented/proven. Therefore R5 remains **Partial / Not Ready** for full P058 conformance and broad release.
