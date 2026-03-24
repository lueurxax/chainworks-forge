# Consolidated Review

## 0. Review Mode and Evidence Summary
- Mode used: `full-review` with product overlay
- Evidence completeness: `Partial`
- Documents / repo inputs reviewed:
  - [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md)
  - [005-goose-server-transport-adapter.md](../proposals/005-goose-server-transport-adapter.md)
  - [005-operator-experience-reports-recovery-and-run-comparison.md](../proposals/005-operator-experience-reports-recovery-and-run-comparison.md)
  - [006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md](../proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md)
  - [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md](../proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md)
  - [004-005-sequential-implementation-consistency-evidence-pack.md](004-005-sequential-implementation-consistency-evidence-pack.md)
- External sources reviewed:
  - [block/goose `agent.rs`](https://github.com/block/goose/blob/main/crates/goose-server/src/routes/agent.rs)
  - [block/goose `reply.rs`](https://github.com/block/goose/blob/main/crates/goose-server/src/routes/reply.rs)
- Build/run attempts:
  - `RUN-01` fresh macOS build passed
  - `RUN-02` focused live / transport / resume test slice failed at compile time
  - `RUN-03` focused UI shell slice failed at compile time before launch
- Screenshots captured:
  - no fresh current-round screenshots
  - reused shell screenshots exist only as partial-freshness evidence (`SCR-01`..`SCR-03`)
- Code areas inspected:
  - [ExecutionService.swift](../../Chainworks%20Forge/Engine/ExecutionService.swift)
  - [GooseServerTransport.swift](../../Chainworks%20Forge/Engine/GooseServerTransport.swift)
  - [ContentView.swift](../../Chainworks%20Forge/ContentView.swift)
  - [Run.swift](../../Chainworks%20Forge/Models/Run.swift)
  - [Artifact.swift](../../Chainworks%20Forge/Models/Artifact.swift)
  - [IdeaListView.swift](../../Chainworks%20Forge/Views/IdeaListView.swift)
  - [ResumeManagerTests.swift](../../Chainworks%20ForgeTests/ResumeManagerTests.swift)
  - [EndToEndTests.swift](../../Chainworks%20ForgeTests/EndToEndTests.swift)
- Remaining assumptions:
  - the canonical current Proposal 004 baseline is [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), not the deleted `docs/proposals/004-live-provider-execution-slice.md`
  - the user’s request materially asks about scope sequencing, so the product overlay is in scope
- Remaining blockers:
  - no fresh UI proof from current HEAD
  - shared test target is red on transport-API migration drift
  - proposal numbering / path migration is currently ambiguous

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `Medium`
- Release blockers:
  1. Proposal 005 transport does not currently preserve Proposal 004’s authoritative read-only acknowledgement contract.
  2. Proposal 005 operator assumes implementation/release capabilities that the canonical 004 live slice explicitly excludes and Proposal 007 still owns.
  3. The repo currently contains two different Proposal 005 drafts, so downstream dependencies are ambiguous by ID.
- Top risks:
  1. A `goose_server` rollout can appear to satisfy the 004 safety gate while relying on a locally synthesized acknowledgement.
  2. Operator work can start against the wrong baseline and force report/recovery scope onto the wrong proposal.
  3. Reviews, audits, and later proposals can resolve “Proposal 005” inconsistently.
- Top opportunities:
  1. The sequential chain can be stabilized quickly if numbering, safety ownership, and the 004→005 operator handoff are corrected before more implementation lands.
  2. The transport and operator proposals can still compose cleanly if operator scope is split into a thin first phase and a later richer phase.
  3. The existing app target already builds, and the live proposal-loop shell still exists as a concrete anchor for follow-on work.

## 2. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Red | Medium | Partial | 0 | 1 | 1 | 0 |
| UX | Amber | High | Partial | 0 | 1 | 2 | 0 |
| iOS Architecture | Red | High | Partial | 0 | 2 | 1 | 0 |
| Product | Red | High | Partial | 0 | 2 | 2 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings
- Finding ID: `UI-GAP-01`
  Severity: `High`
  Evidence IDs: `RUN-03`, `SCR-04`, `OPS-02`, `BASE-01`
  Why it matters: the current round did not produce fresh UI evidence. The reused screenshots are explicitly stale relative to later `IdeaListView.swift` / UI-test changes, and the current UI run failed before launch. That means any UI sign-off on the sequential flow would be partially speculative.
  Recommended fix: repair the test-target drift in [EndToEndTests.swift](../../Chainworks%20ForgeTests/EndToEndTests.swift#L309) and [ResumeManagerTests.swift](../../Chainworks%20ForgeTests/ResumeManagerTests.swift#L275), then recapture entry, main-path, confirmation, error, empty, and recovery states from current HEAD.
  Acceptance criteria: a current-round UI run succeeds and exports fresh screenshots for the reviewed flow.
  Confidence: `High`

- Finding ID: `UI-01`
  Severity: `Medium`
  Evidence IDs: `DOC-04`, `CODE-04`, `SCR-01`, `SCR-02`
  Why it matters: the operator proposal introduces a command-center style UI, but the current shell still centers the product on peer tabs rather than one dominant operator surface. That makes the proposed `Runs Home` hierarchy feel additive instead of foundational.
  Recommended fix: make `Runs Home` the primary landing surface and demote report/recovery/comparison into contextual actions from that surface.
  Acceptance criteria: one obvious default operator entry point groups attention states above the fold.
  Confidence: `Medium`

### 3.2 UX Findings
- Finding ID: `UX-002`
  Severity: `High`
  Evidence IDs: `DOC-04`, `CODE-05`, `BASE-01`
  Why it matters: Proposal 005 operator says every terminal run gets `run_report.md` / `run_report.json`, but the same proposal also allows same-run retry, resume, and stage/agent recovery. Without immutable or versioned report semantics, the operator can read a “final” record that no longer matches the run after recovery.
  Recommended fix: split immutable historical reports from mutable current summaries, or version reports by recovery cycle / stable checkpoint.
  Acceptance criteria: recovery never silently overwrites a historical report, and the UI states clearly whether a report is immutable history or latest summary.
  Confidence: `High`

- Finding ID: `UX-001`
  Severity: `Medium`
  Evidence IDs: `DOC-04`, `CODE-04`, `CODE-05`, `BASE-01`
  Why it matters: the proposed universal row actions (`Open`, `Open gate`, `Retry`, `Compare`, `View report`) are not universally executable from a single row. `Compare` needs a second run and `View report` depends on report availability, so the front door promises actions that can dead-end.
  Recommended fix: make row actions contextual by status/section, or add explicit picker flows so comparison and report viewing are always well-formed.
  Acceptance criteria: every visible row action is executable from that row without hidden prerequisites.
  Confidence: `High`

- Finding ID: `UX-003`
  Severity: `Medium`
  Evidence IDs: `DOC-02`, `DOC-03`, `DOC-04`, `CODE-01`, `CODE-03`
  Why it matters: the operator proposal overstates what the user should believe is already safe and available after 004 by implying implementation/release-side readiness and by not clarifying the read-only-ack downgrade in `goosed` mode.
  Recommended fix: rewrite the 004→005 bridge text so 004 is described strictly as the proposal-loop live baseline, and any side-effect or release-adjacent behavior is explicitly deferred.
  Acceptance criteria: the docs consistently label 004 as proposal-loop only, and later operator features are explicitly tagged with the prerequisite they need.
  Confidence: `Medium`

### 3.3 iOS Architecture Findings
- Finding ID: `ARCH-001`
  Severity: `High`
  Evidence IDs: `DOC-02`, `DOC-03`, `CODE-03`, `WEB-01`, `WEB-02`
  Why it matters: the canonical live-slice reference requires a fail-closed read-only acknowledgement before launch, but `goosed` does not provide that proof and [GooseServerTransport.swift](../../Chainworks%20Forge/Engine/GooseServerTransport.swift#L123) currently synthesizes a positive acknowledgement locally. That turns a compatibility shim into an apparent safety proof.
  Recommended fix: either block `goose_server` mode until a real authoritative acknowledgement exists, or rewrite the 004 safety contract and the transport proposal to state explicitly that this mode is not backed by server-verified policy acknowledgement.
  Acceptance criteria: live `goose_server` runs cannot start unless a real documented acknowledgement source exists, or the docs/tests clearly label the mode as non-authoritative.
  Confidence: `High`

- Finding ID: `ARCH-002`
  Severity: `High`
  Evidence IDs: `DOC-02`, `DOC-04`, `DOC-05`, `CODE-04`, `CODE-05`
  Why it matters: the operator proposal is sequenced against capabilities that the canonical 004 baseline excludes. [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md) is proposal-loop only and forbids writable worktrees / git / release side effects, while Proposal 007 still introduces those repo-backed flows later. That boundary mismatch will push operator/report/recovery work onto the wrong upstream contract.
  Recommended fix: narrow Proposal 005 operator to the actual 004 baseline, and defer worktree/release-adjacent recovery or comparison behavior to the later proposal that truly owns those flows.
  Acceptance criteria: no section of Proposal 005 operator claims 004 includes writable worktrees or release side effects.
  Confidence: `High`

- Finding ID: `ARCH-003`
  Severity: `Medium`
  Evidence IDs: `DOC-03`, `RUN-02`, `RUN-03`, `CODE-07`, `CODE-08`
  Why it matters: transport-API migration already broke shared test factories and live/recovery test call sites, so the sequential chain is not protected by a green baseline. Operator work on top of this will compound drift.
  Recommended fix: add an explicit cross-proposal migration gate to the transport/operator docs: before operator work proceeds, the live proposal-loop, end-to-end fixture, and waiting-approval recovery suites must compile and pass.
  Acceptance criteria: the targeted live/recovery suites pass on current HEAD before the next proposal starts implementation.
  Confidence: `High`

### 3.4 Product Findings
- Finding ID: `PROD-001`
  Severity: `High`
  Evidence IDs: `DOC-06`, `OPS-01`, `DOC-03`, `DOC-04`
  Why it matters: the repo currently contains two different Proposal 005 drafts, while downstream docs such as Proposal 006 still depend on “Proposal 005” singular. That makes the roadmap and dependency graph non-deterministic.
  Recommended fix: renumber one of the drafts and update every downstream reference, review file, and dependency table to a unique canonical ID.
  Acceptance criteria: every downstream reference resolves to exactly one proposal, and no document uses bare `Proposal 005` where two 005 drafts exist.
  Confidence: `High`

- Finding ID: `PROD-002`
  Severity: `High`
  Evidence IDs: `DOC-02`, `DOC-04`, `DOC-05`, `BASE-01`
  Why it matters: the operator proposal claims a stronger inherited product state than the live-slice reference and current shell actually provide. That makes the roadmap read as if operator experience can start from a calmer, richer baseline than exists today.
  Recommended fix: split Proposal 005 operator into a thinner first phase anchored to current proposal-loop artifacts, then defer comparison / broader recovery / notification scope until supporting models and screens exist.
  Acceptance criteria: each operator milestone names the exact upstream models and screens it depends on, and no milestone references absent fields or surfaces.
  Confidence: `High`

- Finding ID: `PROD-003`
  Severity: `Medium`
  Evidence IDs: `DOC-03`, `DOC-04`, `DOC-06`
  Why it matters: the transport proposal has a concrete success condition (“real live run”), while the operator proposal bundles several surfaces without a crisp decision gate to tell later proposals when the phase is “done enough” to unblock 006/007.
  Recommended fix: add explicit leading metrics, guardrails, and phase handoff checkpoints to both proposals.
  Acceptance criteria: each proposal defines a measurable handoff condition to the next proposal.
  Confidence: `Medium`

## 4. Cross-Discipline Conflicts and Decisions
- Conflict: Proposal 004 reference requires backend-acknowledged read-only launch policy, but Proposal 005 transport currently relies on a synthesized acknowledgement in code.
  Tradeoff: keep the current transport path moving quickly versus preserve the original safety contract honestly.
  Decision: do not treat the synthetic acknowledgement as sign-off proof; either add a real authoritative source or downgrade the documented guarantee explicitly.
  Owner: architecture / product

- Conflict: Proposal 005 operator describes a baseline that already includes worktree/release-side capability, while Proposal 007 still owns repo-backed implementation/release.
  Tradeoff: broaden 004/005 scope versus keep proposal boundaries narrow and composable.
  Decision: keep 004 narrow and rewrite 005 operator to target the actual 004 baseline.
  Owner: product / architecture

- Conflict: two drafts currently share Proposal ID `005`.
  Tradeoff: preserve current filenames versus keep the roadmap and dependency graph unambiguous.
  Decision: renumber before more downstream proposals or reviews are published.
  Owner: product / docs

## 5. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Renumber one of the two Proposal 005 drafts and update every downstream dependency reference | Product | Docs owner | Immediate | none | zero ambiguous `Proposal 005` references remain | `PROD-001`, `ARCH-003` |
| P0 | Decide and document whether `goose_server` mode has an authoritative read-only acknowledgement source or must remain fail-closed | Architecture | Runtime owner | Immediate | `goosed` contract decision | no live launch can pass on synthetic acknowledgement alone | `ARCH-001`, `PROD-002` |
| P0 | Rewrite the 004→005 operator handoff so Proposal 005 operator no longer claims implementation/release capabilities that 004 excludes and 007 later owns | Product / Architecture | Proposal author | Immediate | canonical 004 reference | operator scope matches actual baseline | `ARCH-002`, `UX-003`, `PROD-002` |
| P1 | Define immutable/versioned report semantics before implementing `RunReportBuilder` and recovery actions | UX / Architecture | Proposal author | Next | report model decision | same-run recovery cannot invalidate a historical report | `UX-002` |
| P1 | Repair shared test-target drift in [ResumeManagerTests.swift](../../Chainworks%20ForgeTests/ResumeManagerTests.swift#L275) and [EndToEndTests.swift](../../Chainworks%20ForgeTests/EndToEndTests.swift#L309), then recapture fresh UI evidence | UI / Architecture | Runtime owner | Next | transport API settled | fresh screenshots and green targeted live/recovery suites | `UI-GAP-01`, `ARCH-003` |
| P2 | Make Runs Home the dominant operator surface and contextualize row actions | UI / UX | Design / app shell owner | Later | operator phase 1 scope | one obvious operator landing surface, no dead-end quick actions | `UI-01`, `UX-001` |

## 6. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Safety contract | Whether live `goose_server` launches prove a real read-only acknowledgement source | percent of live launches with authoritative ack evidence | any launch that passes only on synthetic/local ack | before transport proposal sign-off | hold if the only ack is locally synthesized |
| Proposal graph | Whether dependencies resolve unambiguously | count of unique, resolvable proposal references | any bare `Proposal 005` reference after renumbering decision | before 006/007 follow-on edits | hold if one reference can point to two drafts |
| Report lifecycle | Whether recovery can preserve trustworthy history | percent of recovered runs with explicit report versioning | any overwrite of a historical report artifact | before `RunReportBuilder` implementation | hold if “final report” can become stale silently |
| UI evidence | Whether current-head operator flow is freshly observable | fresh screenshot coverage for entry, main, confirmation, error, empty, recovery | reused/stale screenshots only | after test-target repair | hold if current-round UI run fails before capture |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: The current round did not produce fresh UI screenshots because the UI test slice failed at compilation before launch (`RUN-03`, `SCR-04`).
- `GAP-02`: The shared live/recovery test baseline is currently broken by transport-API migration drift (`RUN-02`, `CODE-07`, `CODE-08`).
- `GAP-03`: The user-supplied Proposal 004 path is deleted from the current working tree, and the doc migration to [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md) is not yet reflected everywhere.

### Open Questions
- `QUESTION-01`: Should `goose_server` mode remain unsupported until a real read-only acknowledgement source exists, or should the canonical 004 safety contract be downgraded explicitly?
- `QUESTION-02`: Which draft should keep Proposal ID `005`, and which should be renumbered?
- `QUESTION-03`: Should `Runs Home` replace the current landing surface or be introduced as a separate operator hub after a thinner first operator phase ships?

## Evidence Gap Review Fallback
- What was attempted:
  - refreshed the current canonical 004 baseline via [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md)
  - re-read the two Proposal 005 drafts plus downstream 006/007 dependencies
  - ran a fresh macOS build and fresh targeted test/UI attempts
  - checked screenshot freshness against current file mtimes
  - ran UI, UX, architecture, and product specialist passes against one shared evidence pack
- What is missing:
  - fresh current-head UI screenshots for the reviewed sequential flow
  - a green shared live/recovery test baseline after the transport API changes
- Blockers:
  - [ResumeManagerTests.swift](../../Chainworks%20ForgeTests/ResumeManagerTests.swift#L275) still misses `transportAPI` in live-runtime fixtures
  - [EndToEndTests.swift](../../Chainworks%20ForgeTests/EndToEndTests.swift#L309) still calls an obsolete `FixtureGooseTransport` initializer
  - the docs graph still has duplicate `Proposal 005` numbering and a deleted 004 proposal path
- Confidence: `Medium`
- What can still be said with partial confidence:
  - the current triad is not safely aligned for sequential implementation yet
  - the main blockers are not cosmetic; they are dependency, safety-contract, and sequencing-ownership issues
  - the report lifecycle problem and universal quick-action problem remain real even before implementation
- What evidence is required to finish the full review:
  - fresh current-head UI screenshots after test-target repair
  - green targeted live proposal-loop + recovery suites
  - a resolved numbering/path graph for the canonical 004/005 documents
