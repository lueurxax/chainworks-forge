# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/044-post-approval-task-execution-and-release-gate-completion.md` | 2026-04-14 | High | Current draft now closes the old dependency, end-state-owner, proof-lane, multi-task `then`, and summary-sync blockers. | Review could keep using stale blockers instead of current proposal reality. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-14 | High | Proposal review should ground itself in the stable local baseline chain. | Review could drift into proposal-only reasoning. | Intake baseline. |
| DOC-03 | `docs/reference/full-mvp-delivery.md` | 2026-04-14 | High | Stable repo-backed delivery keeps explicit ordering for `state_9`, explicit manual release at `state_11`, and terminal receipt/report production at `state_12`. | Proposal must preserve all three surfaces. | Canonical delivery reference. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-14 | High | Current daemon still needs all of the scoped changes described by `P044`; baseline is sufficient for proposal-readiness review. | Grounds the review in current daemon reality. | Daemon behavior baseline. |
| DOC-05 | `docs/reference/workflow-execution-engine.md` | 2026-04-14 | High | Stable engine docs remain relevant for parity, especially around current execution ownership and transition handling. | Prevents proposal-only reasoning. | Execution-engine baseline. |
| DOC-06 | `docs/reference/runtime-contract.md` | 2026-04-14 | High | Approval / retry / side-effect status boundaries are already stable enough for this review. | Avoids inventing missing evidence gaps around retry semantics. | Runtime contract baseline. |
| DOC-07 | `docs/reference/045-deterministic-release-operations.md` | 2026-04-14 | High | Adjacent deterministic-release slice now correctly depends on `P044`; the old circular dependency finding is stale. | Prevents stale dependency findings from surviving into this round. | Direct dependency seam. |
| DOC-08 | `docs/reference/yaml-dsl-parser.md` | 2026-04-14 | High | The DSL contract defines both `sequence` and `then` as ordered structures that the proposal now explicitly preserves. | Confirms the latest draft matches canonical YAML semantics. | Canonical YAML semantics. |
| DOC-09 | `docs/reference/test-gates.md` | 2026-04-14 | High | Current repo proof model is explicit named gates, and the new draft now explicitly owns `proposal-044`. | Confirms the old proof-lane blocker is stale. | Proof ownership baseline. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake routing | 2026-04-14 | High | Used as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/full-mvp-delivery.md` | Reused | repo-backed delivery / manual release / implementation-review order | 2026-04-14 | High | Fresh and central to this slice. | Main workflow baseline. |
| BASE-03 | `docs/reference/rust-control-plane.md` | Reused | daemon state machine / end-state model | 2026-04-14 | High | Fresh and directly relevant. | Rust daemon baseline. |
| BASE-04 | `docs/reference/workflow-execution-engine.md` | Reused | stable engine semantics | 2026-04-14 | High | Fresh and directly relevant. | App/runtime comparison baseline. |
| BASE-05 | `docs/reference/runtime-contract.md` | Reused | stage / approval / retry contract | 2026-04-14 | High | Fresh and directly relevant. | Retry / settlement baseline. |
| BASE-06 | `docs/reference/yaml-dsl-parser.md` | Reused | YAML run-block semantics | 2026-04-14 | High | Fresh and directly relevant. | DSL semantics baseline. |
| BASE-07 | `docs/reference/test-gates.md` | Reused | proposal-proof governance | 2026-04-14 | High | Fresh and directly relevant. | Verification ownership baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - daemon handling of `run_after_approval`
  - manual-release stage lifecycle after approval
  - effective-task resolution for post-approval running stages
  - generalized N-phase ordering for YAML run blocks
  - retry / re-approval behavior after post-approval failure
  - end-state-with-run execution for `state_12`
- Out of scope:
  - deterministic release-service implementation itself
  - GraphQL/MCP northbound design
  - broader thin-client migration
- Assumptions:
  - review mode is `proposal-readiness`
  - local proposal/docs/code evidence is sufficient
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | Manual release gate / run detail | Baseline | 2026-04-14 | High | The product already has an explicit manual release gate inside the repo-backed run shell. | Proposal could mis-treat release as only a daemon-internal concern. | Operator entry point. |
| NAV-02 | Workflow state `state_9_implementation_reviewed` | Baseline + current repo | 2026-04-14 | High | Canonical workflow still relies on an ordered `then` chain: implementation audit, then pre-push review, then aggregation. | Proposal must preserve this non-regression surface. | Main non-regression seam. |
| NAV-03 | Workflow state `state_11_manual_release` | Proposal + current repo | 2026-04-14 | High | Canonical workflow still defines ordered post-approval release tasks. | Proposal must preserve this release sequencing contract. | Primary state-11 seam. |
| NAV-04 | Workflow state `state_12_workflow_complete` | Baseline + current repo | 2026-04-14 | High | `state_12` still owns `finalize_run_and_produce_receipts`, not just a terminal marker. | Proposal must preserve this terminal execution contract. | Terminal-state seam. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/workflow/src/definition.rs` | workflow definition | YAML schema owner | 2026-04-14 | High | `WorkflowState` already owns `run_after_approval`, and `RunBlock` already models `sequence`, `parallel`, and `then`. | Schema is not a blocker. | Source-schema seam. |
| MAP-02 | `control-plane/crates/workflow/src/plan.rs` | compiled workflow | execution-time state owner | 2026-04-14 | High | `CompiledState` already carries `post_approval_tasks`, and `CompiledTask.phase` is flexible enough for N-phase semantics. | Confirms model capacity for the proposal. | Compiled-phase seam. |
| MAP-03 | `control-plane/crates/workflow/src/compiler.rs` | compiler | phase mapping | 2026-04-14 | High | Current code still needs the change, and the draft now correctly specifies incrementing phases for both `sequence` and multi-task `then`. | Confirms the proposal’s target is coherent. | Main compiler seam. |
| MAP-04 | `control-plane/crates/workflow/tests/integration.rs` | workflow tests | compile-time proof | 2026-04-14 | High | Current tests prove workflow shape exists; the draft now additionally names the missing proof slices. | Confirms proof intent is coherent. | Existing compile proof. |
| MAP-05 | `control-plane/crates/engine/src/command_handler.rs` | command path | approval mutation owner | 2026-04-14 | High | Current code still settles manual gates immediately unless changed; the draft correctly scopes the conditional `Running` fix. | Confirms the proposal addresses the right owner seam. | Approval mutation seam. |
| MAP-06 | `control-plane/crates/engine/src/orchestrator.rs` | orchestrator | stage lifecycle / enqueue / end-state handling | 2026-04-14 | High | Current code still only understands phase 0/1 and still short-circuits end states, but the draft now coherently specifies both fixes. | Confirms proposal-level ownership is now clear. | Main runtime owner seam. |
| MAP-07 | `control-plane/crates/engine/tests/integration.rs` | daemon tests | approval / retry parity | 2026-04-14 | High | Current daemon tests do not yet prove this slice, and the draft now names the required proof set. | Confirms proof intent is coherent. | Proof seam. |
| MAP-08 | `Chainworks Forge/Engine/RunPlanCompiler.swift` | Swift baseline | phase-preserving compile model | 2026-04-14 | High | Current Swift compiler preserves execution phases structurally. | Confirms the intended ordering model is already canonical. | Baseline parity seam. |
| MAP-09 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | Swift baseline | approval-resume / terminal execution parity | 2026-04-14 | High | Current Swift orchestrator preserves ordered run-block execution and only transitions after execution completes. | Confirms the intended orchestration owner model. | Baseline behavior source. |
| MAP-10 | `examples/workflows/full-mvp-live.yaml` | workflow fixture | canonical release path and review path | 2026-04-14 | High | `state_9` has a three-step `then` chain; `state_11` has ordered release tasks; `state_12` has an end-state `run` block. | Confirms the proposal now matches the canonical workflow. | Canonical workflow contract. |
| MAP-11 | `scripts/test-gate.sh` | verification | repository-owned proof inventory | 2026-04-14 | High | No `proposal-044` gate exists today in code, and the draft now explicitly owns adding it. | Confirms the proposal-level proof contract is now explicit. | Gate inventory seam. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | dependency chain with `P045` | Proposal + adjacent proposal | 2026-04-14 | High | `P044` now correctly says `P045` depends on it; the old circular dependency blocker is stale. | No live proposal-first conflict remains here. | Cleared seam. |
| INT-02 | end-state completion owner | Baseline + current repo | 2026-04-14 | High | Canonical workflow still needs `state_12.run`, and `P044` now explicitly owns that fix. | No live proposal-first conflict remains here. | Cleared seam. |
| INT-03 | proof-lane ownership | Baseline + proposal | 2026-04-14 | High | Current repo expects explicit named proposal gates, and the new draft now defines `proposal-044`. | No live proposal-first conflict remains here. | Cleared seam. |
| INT-04 | multi-task `then` ordering | Baseline + current repo + proposal | 2026-04-14 | High | The draft now explicitly serializes multi-task `then`, updates acceptance, and includes the matching proof test. | No live proposal-first conflict remains here. | Cleared seam. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Approval granted on manual release | Specified | DOC-01, MAP-05, MAP-06, MAP-10 | `ApproveStage`, orchestrator, workflow state 11 | Proposal-level owner is explicit. |
| Post-approval effective task resolution | Specified | DOC-01, MAP-02, MAP-06 | running-stage settlement path | Proposal-level owner is explicit. |
| Post-approval canonical sequence order | Specified | DOC-01, MAP-03, MAP-06, MAP-08, MAP-09, MAP-10 | compiler + orchestrator | State_11 path is fully covered. |
| Existing `state_9` `then` review chain | Specified | DOC-01, DOC-03, DOC-08, MAP-03, MAP-06, MAP-10 | compiler + orchestrator + workflow | Proposal now explicitly covers this non-regression surface. |
| Transition to state 12 | Specified | DOC-01, MAP-10 | `exists('git_push_receipt')` transition | Proposal owns the state_11 side of the handoff. |
| State 12 receipt/report execution | Specified | DOC-01, DOC-04, MAP-06, MAP-10 | end-state handling | Proposal now explicitly owns this fix. |
| Retry after post-approval failure | Specified | DOC-01, DOC-06, MAP-05, MAP-06 | stage retry / approval flow | Fresh-approval retry semantics are explicit and aligned. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | current repo | compile-time workflow validation | current tests prove workflow shape exists | add proof for ordered multi-task `then`, post-approval release sequencing, and end-state execution | 2026-04-14 | High | Proposal-level proof intent is sufficient. |
| TEST-02 | current repo | state-machine / approval parity | current daemon integration tests cover simple approval/retry semantics | add focused `state_9`, `state_11`, and `state_12` proof slices | 2026-04-14 | High | Proposal-level proof intent is sufficient. |
| TEST-03 | current repo | gate governance | repo uses named lanes in `test-gates.md` / `test-gate.sh` | add `proposal-044` lane and document it | 2026-04-14 | High | Proposal-level proof ownership is explicit. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | dependency chain | old blocker: `P044` depends on `P045` | current draft says `P045` depends on `P044` | 2026-04-14 | High | Previous dependency finding is stale. |
| REAL-02 | terminal-state completion | old blocker: `P044` overclaims `state_12` | current draft explicitly owns end-state-with-run execution | 2026-04-14 | High | Previous end-state finding is stale. |
| REAL-03 | proof contract | old blocker: no `proposal-044` lane | current draft explicitly defines `proposal-044` | 2026-04-14 | High | Previous proof-lane finding is stale. |
| REAL-04 | `then` ordering | old blocker: multi-task `then` still shared-phase | current draft explicitly increments `then` phases and adds `state_9` proof | 2026-04-14 | High | Previous multi-task-then finding is stale. |
| REAL-05 | summary sync | old blocker: summary sections lagged the detailed design | current draft now synchronizes scope, file summary, and gate scope | 2026-04-14 | High | Previous summary-sync finding is stale. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, MAP-05, MAP-06, MAP-10 | The core state_11 orchestration bug is real and well framed. |
| Scope boundaries | Specified | DOC-01, DOC-07, INT-01, INT-02, INT-03, INT-04 | No live proposal-first contradiction remains. |
| Reusable baseline coverage | Specified | BASE-02, BASE-03, BASE-04, BASE-06, BASE-07 | Baseline coverage is sufficient for a proposal-first verdict. |
| State handling | Specified | G matrix | State_9, state_11, and state_12 are all covered. |
| Data / execution contract | Specified | MAP-02, MAP-03, MAP-06 | N-phase and end-state contracts are coherent. |
| Retry / recovery semantics | Specified | DOC-06, MAP-05, MAP-06 | Fresh-approval retry semantics are clear. |
| Testing strategy | Specified | TEST-01, TEST-02, TEST-03 | Proposal-level proof contract is explicit. |
| Dependencies / integration points | Specified | DOC-07, INT-01, INT-02, INT-03, INT-04 | Adjacent dependencies and handoffs are coherent. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P044` is intended to be the generalized orchestration fix, not only a release-gate patch.
- ASSUMP-02: The DSL phrase "`then` — sequential tasks after parallel fan-out completes" remains authoritative.
- QUESTION-01: None.
- BLOCKER-01: None.

## L. Research Triggers / External Questions
No external research trigger was required. Local proposal/docs/code/baseline evidence is sufficient for a proposal-readiness verdict.
