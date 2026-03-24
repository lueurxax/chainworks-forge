# Consolidated Review

## 0. Review Mode and Evidence Summary
- Mode used: `full-review` with product overlay
- Evidence completeness: `Partial`
- Documents / repo inputs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/post-007-mvp-readiness-evidence-pack.md`
- External sources reviewed:
  - none required
- Build/run attempts:
  - green app build on current HEAD
  - green focused macOS UI slice on current HEAD
- Screenshots captured:
  - `REQ011_Approvals`
  - `P004_NonHappy_MissingRuntime`
  - `REQ011_RunProgress_Entry`
  - `REQ011_RunProgress_Overview`
  - `REQ011_RunProgress_Sections`
- Code areas inspected:
  - current app shell and operator surfaces
  - current persistence/runtime models
  - absence proofs for Proposal 006 provider/settings files
  - absence proofs for Proposal 007 repo-backed delivery files
- Remaining assumptions:
  - this review answers what should be in the proposal after 007, assuming 007 lands substantially as drafted
- Remaining blockers:
  - no live end-to-end Proposal 007 runtime exists yet
  - no repeated manual-vs-app timing cohort exists yet

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `Medium`
- Release blockers:
  - Proposal 007 does not by itself close the PS success metric or final MVP sign-off.
  - The MVP provider boundary is inconsistent across the PS/runtime contract and Proposal 006.
  - Several PS-level contracts are still unresolved or explicitly `[TBD]`.
- Top risks:
  1. The roadmap stops at a believable dogfood demo instead of a PS-backed MVP.
  2. Provider scope drifts from the PS-required pair (`codex`, `claude_code`) into an unnecessarily wider MVP matrix.
  3. MVP remains non-testable because attachment policy, output-retrieval SLO, cost granularity, and approval-resume behavior are still undecided.
- Top opportunities:
  1. Make the next proposal a narrow MVP hardening/sign-off slice instead of another capability expansion.
  2. Freeze the final MVP boundary and reduce late-stage scope churn.
  3. Turn dogfood evidence, timing, and recovery proof into the actual MVP go/no-go mechanism.

## 2. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | Medium | Partial | 0 | 1 | 1 | 0 |
| UX | Amber | Medium | Partial | 0 | 2 | 1 | 0 |
| iOS Architecture | Amber | Medium | Partial | 0 | 1 | 1 | 0 |
| Product | Amber | High | Partial | 0 | 2 | 0 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings
- Finding ID: `UI-008-001`
  Severity: `High`
  Evidence IDs: `DOC-03`, `DOC-01`, `CODE-04`, `SCR-01`, `SCR-03`
  Why it matters: Proposal 007 already requires blocked implementation/release recovery and release-gate re-entry, but the current shell evidence only proves generic operator surfaces. The next proposal must explicitly add the repo-backed recovery/re-entry UI needed to satisfy the PS rule that approval/side-effect stages never silently auto-resume.
  Recommended fix: make the post-007 proposal own one operator-visible recovery/re-entry surface for blocked implementation review and blocked release, plus a completed-run export hub for dogfood evidence.
  Acceptance criteria:
  - a blocked implementation review or blocked release can be recovered or cancelled from one visible UI surface
  - the surface shows blocker reason, preserved receipts, diff/test context, and exact next action
  - terminal dogfood runs expose one completion/export screen with report, receipts, evidence-pack status, elapsed time, and cost
  - blocked, re-entry, and terminal export states are screenshot-tested
  Confidence: `Medium`

### 3.2 UX Findings
- Finding ID: `UX-008-001`
  Severity: `High`
  Evidence IDs: `DOC-03`, `DATA-01`, `DOC-01`
  Why it matters: Proposal 007 is intentionally framed as the first believable full-loop dogfood slice, not the final MVP sign-off. The PS still defines success as a 50% reduction in manual orchestration time per idea, which a single sample-repo dogfood proof does not establish.
  Recommended fix: make the next proposal an MVP closure/hardening proposal with explicit benchmark protocol, stop/go criteria, one messier real-world repo target, and required happy-path plus non-happy-path evidence packs.
  Acceptance criteria:
  - the proposal defines the fixed benchmark cohort and manual-vs-app comparison method
  - it measures time to proposal approval, implementation approval, and final release decision
  - it requires at least one happy-path and one recovered non-happy-path run
  - it names the final MVP readiness gate and failure-hold criteria
  Confidence: `Medium`

- Finding ID: `UX-008-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`
  Why it matters: the PS still leaves attachment-file support, cost granularity, approval-gate relaunch behavior, and the active-output retrieval SLO unresolved. That makes the final MVP contract hard to test at exactly the moment it should become crisp.
  Recommended fix: the next proposal should close every remaining PS open question or explicitly defer it, and replace the `[TBD]` retrieval target with a measurable number and verification method.
  Acceptance criteria:
  - supported attachment file types are explicitly listed
  - report cost granularity is fixed
  - waiting-approval relaunch behavior is fixed
  - active-output retrieval has a numeric target and a test
  Confidence: `High`

### 3.3 iOS Architecture Findings
- Finding ID: `ARCH-008-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-02`, `DOC-04`, `QUESTION-01`
  Why it matters: the PS and runtime contract still define MVP providers as `codex` and `claude_code`, while Proposal 006 expands the MVP-ready matrix to Gemini. That is a late-stage contract drift that increases implementation and test surface just as the roadmap should be converging.
  Recommended fix: the next proposal must freeze one canonical MVP provider boundary and align the PS, runtime contract, Proposal 006 wording, UI copy, and acceptance tests to it. Based on current repo contracts, the least-surprising decision is Codex + Claude for MVP, Gemini post-MVP.
  Acceptance criteria:
  - one provider list is canonical across PS, runtime contract, proposals, and tests
  - no MVP acceptance path depends on Gemini
  - run-start, preflight, and diagnostics copy use the same provider boundary
  Confidence: `High`

- Finding ID: `ARCH-008-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `DOC-03`, `DATA-01`
  Why it matters: Proposal 007 proves a repo-backed control-plane path, but it still does not own the final benchmark harness or MVP sign-off instrumentation required by the PS measurement plan. Without a dedicated owner, the repo can ship a technically complete flow without a defensible “MVP achieved” decision.
  Recommended fix: put the benchmark harness, timing instrumentation, and final sign-off/export rules into the next proposal rather than spreading them informally across dogfood runs.
  Acceptance criteria:
  - the next proposal defines how the benchmark data is collected, stored, reported, and reviewed
  - completed runs expose the three PS checkpoint timings plus total elapsed time and MVP pass/fail
  Confidence: `Medium`

### 3.4 Product Findings
- Finding ID: `PROD-008-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-03`, `DATA-01`, `BASE-01`
  Why it matters: the next proposal after 007 should not be another capability slice. Proposal 007 itself already says the likely next step is dogfood hardening, followed later by Steward and backend extraction. That matches the PS more closely than expanding platform scope.
  Recommended fix: make the next proposal a narrow “MVP Hardening and Sign-Off” slice focused on validation, scope freeze, evidence export, and launch-readiness decisions.
  Acceptance criteria:
  - the proposal explicitly excludes Steward activation, Temporal/backend extraction, and new provider-family expansion
  - it owns MVP sign-off criteria and roadmap handoff to post-MVP work
  Confidence: `High`

- Finding ID: `PROD-008-002`
  Severity: `High`
  Evidence IDs: `DOC-03`, `DOC-01`, `CODE-01`, `SCR-03`
  Why it matters: 007’s evidence pack builder and sample repo are necessary but not sufficient. The PS outcome requires the product to prove repeatability and reduced coordination overhead on more than a clean sample-repo narrative.
  Recommended fix: the next proposal should require one messier real-world repo target, final export/support artifacts, and a documented go/no-go review packet for MVP readiness.
  Acceptance criteria:
  - one second, messier repo target is part of the benchmark cohort
  - every benchmark run exports a standardized review packet
  - MVP review cannot sign off without that packet
  Confidence: `High`

## 4. Cross-Discipline Conflicts and Decisions
- Conflict: Proposal 006 widens provider scope to Gemini while the PS/runtime contract still define a narrower MVP pair.
  Tradeoff: wider provider coverage may help future pilot breadth, but it increases ambiguity and validation cost for MVP sign-off.
  Decision: freeze MVP to Codex + Claude Code in the next proposal unless the PS itself is intentionally revised.
  Owner: post-007 proposal author

- Conflict: Proposal 007’s “believable dogfood” framing can be mistaken for full MVP closure.
  Tradeoff: treating 007 as full sign-off is faster, but it skips the PS measurement plan and unresolved contract decisions.
  Decision: add a narrow MVP hardening/sign-off proposal before any Steward or backend-scaling work.
  Owner: post-007 proposal author

## 5. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Define Proposal 008 as `MVP Hardening and Sign-Off`, not a new platform/capability slice | Product | Engineer | Next proposal | Proposal 007 lands | Proposal text clearly scopes validation/closure work only | `PROD-008-001`, `UX-008-001` |
| P0 | Freeze the MVP provider boundary to one canonical list and align docs/tests/UI | Architecture | Engineer | Next proposal | Proposal 006 wording update or explicit override | No MVP acceptance path depends on Gemini | `ARCH-008-001`, `PROD-008-001` |
| P0 | Make Proposal 008 own the PS measurement plan and final MVP go/no-go gate | Product | Engineer | Next proposal | Proposal 007 dogfood path exists | Benchmark cohort proves or disproves the 50% time-reduction target | `UX-008-001`, `ARCH-008-002`, `PROD-008-002` |
| P1 | Close PS open questions: attachment types, cost granularity, approval relaunch behavior, retrieval SLO | UX | Engineer | Proposal 008 design | P0 scope freeze | No `[TBD]` or unowned MVP-open-question remains | `UX-008-002` |
| P1 | Add repo-backed blocked-state recovery/re-entry UI and completed-run export hub requirements | UI | Engineer | Proposal 008 implementation | Proposal 007 repo-backed surfaces | Recovery and export states are screenshot-tested and operator-complete | `UI-008-001` |
| P2 | Keep Forge Steward activation and backend extraction explicitly post-MVP | Product | Engineer | After MVP sign-off | P0-P1 complete | No roadmap drift of post-MVP work into the sign-off slice | `PROD-008-001` |

## 6. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| MVP outcome | Manual-vs-app orchestration time on a fixed idea/repo cohort | median time to proposal approval; median time to implementation approval; median time to final release decision | blocked/recovered run rate; incomplete evidence-pack rate | Proposal 008 MVP sign-off review | hold MVP sign-off if the 50% reduction target is not met or evidence packets are incomplete |
| Provider boundary | Readiness of the required MVP providers only | preflight pass rate for Codex + Claude | provider mismatch incidents; unsupported-provider launch attempts | pre-implementation scope freeze review | hold if Gemini or other optional providers remain part of MVP acceptance |
| Operator readiness | Recoverability and report/export completeness | blocked-state recovery success rate; p95 time to open active output/report | UI regression rate; missing receipt/export incidents | repo-backed recovery/export review | hold if blocked/re-entry or export flows still require raw-log reconstruction |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No live end-to-end Proposal 007 runtime exists yet, so the exact Proposal 008 hardening thresholds remain partially inferential.
- GAP-02: No repeated manual-vs-app benchmark dataset exists yet, so MVP sign-off readiness cannot be proven today.

### Open Questions
- QUESTION-01: Should Proposal 006 be narrowed to the PS/runtime-contract MVP provider boundary, or should the PS/runtime contract be consciously widened instead?
- QUESTION-02: Which attachment file types are truly required in MVP validation runs?

