# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/045-deterministic-release-operations.md` | 2026-04-15 | High | Current draft now closes the old input-path, split-routing, structured-failure, proof-lane, preserve-vs-backfill, and terminal-backfill blockers. | Review could keep stale blockers alive. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should anchor to the current stable reference chain. | Review could drift into proposal-only reasoning. | Intake baseline. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-15 | High | Repo-backed delivery is already baseline behavior. | Proposal must port current stable behavior, not invent a new release topology. | Stable baseline anchor. |
| DOC-04 | `docs/reference/full-mvp-delivery.md` | 2026-04-15 | High | Stable delivery contract keeps deterministic release as an explicit post-approval deterministic slice inside the 12-state flow. | Proposal must align to the stable repo-backed delivery contract. | Primary delivery-owner reference. |
| DOC-05 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current daemon still lacks the northbound delivery-config input path and native release executor. | Grounds the proposal in current daemon reality. | Rust daemon baseline. |
| DOC-06 | `docs/reference/test-gates.md` | 2026-04-15 | High | Current repo proof governance is based on named gates, and the draft now correctly owns `proposal-045`. | Prevents stale proof-lane blockers from surviving into this round. | Verification governance baseline. |
| DOC-07 | `docs/archive/proposals/044/044-post-approval-task-execution-and-release-gate-completion.md` | 2026-04-15 | High | `P044` still defines `state_12.finalize_run_and_produce_receipts` as the terminal artifact producer, so `P045` must remain consistent with that handoff. | Confirms `state_12` is still the relevant terminal seam. | Direct dependency context. |
| DOC-08 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | 2026-04-15 | High | Current Swift `persistDeliveryReceiptIfNeeded(...)` requires `currentReleaseResultSummary()` before any terminal backfill occurs, and the draft now explicitly matches that rule. | Confirms the old terminal-backfill blocker is stale. | Main stable owner-chain reference. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake routing | 2026-04-15 | High | Used as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current product/runtime baseline | 2026-04-15 | High | Fresh and directly relevant. | Stable baseline authority. |
| BASE-03 | `docs/reference/full-mvp-delivery.md` | Reused | repo-backed release contract | 2026-04-15 | High | Fresh and central to this slice. | Main safety baseline. |
| BASE-04 | `docs/reference/rust-control-plane.md` | Reused | current daemon executor / start-run owner path | 2026-04-15 | High | Fresh and directly relevant. | Current daemon owner baseline. |
| BASE-05 | `docs/reference/test-gates.md` | Reused | proposal-proof governance | 2026-04-15 | High | Fresh and directly relevant. | Verification ownership baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - deterministic Rust-native release routing for `commit_and_push_to_github`
  - deterministic Rust-native release routing for `build_archive_and_push_connect`
  - executor bypass around ACP for release agents
  - frozen delivery-configuration input path
  - structured `delivery_receipt` persistence
- Out of scope:
  - post-approval orchestration itself
  - broader thin-client migration
  - real App Store Connect upload
- Deferred intentionally:
  - production release mode
  - real App Store Connect communication
- Assumptions:
  - review mode is `proposal-readiness`
  - `P045` continues to preserve current Swift release-owner semantics
- Open questions:
  - none blocking on this pass
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ReleaseGateView` / manual release path | Baseline | 2026-04-15 | High | Release already lives inside the repo-backed operator shell after explicit approval. | Proposal could mis-treat release as only a daemon-internal concern. | Operator entry point. |
| NAV-02 | `state_11_manual_release` release task contract | Baseline + current repo | 2026-04-15 | High | Workflow still defines deterministic git then deterministic publish as ordered post-approval tasks. | Proposal cannot regress to ACP or reorder the two-step release path. | Canonical release path. |
| NAV-03 | `state_12_workflow_complete` terminal finalizer | Proposal + current repo | 2026-04-15 | High | Terminal finalization remains the fallback owner for `delivery_receipt` when earlier release execution did not already write it. | Confirms the terminal handoff stays in scope. | Integration seam. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/engine/src/executor.rs` | Rust daemon | current execution owner | 2026-04-15 | High | `BackgroundExecutor` is the routing seam that `P045` wants to change. | Main native-routing seam. |
| MAP-02 | `control-plane/crates/domain/src/run.rs` | Rust domain | frozen delivery-config storage | 2026-04-15 | High | `Run` already stores `delivery_configuration_json`. | Confirms the old storage blocker is stale. | Delivery-config seam. |
| MAP-03 | `control-plane/crates/domain/src/commands.rs` | Rust domain | start-run input contract | 2026-04-15 | High | `StartRunCmd` still lacks the field today, and the draft now correctly owns adding it. | Confirms the old input-path blocker is stale. | Command seam. |
| MAP-04 | `control-plane/crates/engine/src/command_handler.rs` | Rust daemon | run persistence path | 2026-04-15 | High | `StartRun` still persists `delivery_configuration_json: None` today, and the draft now correctly owns fixing it. | Confirms the old persistence blocker is stale. | Current Rust contradiction seam. |
| MAP-05 | `control-plane/crates/graphql-server/src/schema.rs` | GraphQL northbound | `start_run` mutation surface | 2026-04-15 | High | GraphQL `start_run` exposes no delivery-configuration field today; the draft now correctly owns adding it. | Confirms the old GraphQL blocker is stale. | Northbound seam. |
| MAP-06 | `control-plane/crates/mcp-server/src/tools/runs.rs` | MCP northbound | `runs.start` tool surface | 2026-04-15 | High | MCP `runs.start` likewise exposes no delivery-configuration field today; the draft now correctly owns adding it. | Confirms the old MCP blocker is stale. | Northbound seam. |
| MAP-07 | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | Swift baseline | canonical `delivery_receipt` owner chain | 2026-04-15 | High | Swift terminal backfill calls `persistDeliveryReceiptIfNeeded(...)`, which first requires non-nil `currentReleaseResultSummary()`, and the draft now explicitly mirrors that. | Confirms the old parity blocker is stale. | Main stable parity source. |
| MAP-08 | `docs/archive/proposals/044/044-post-approval-task-execution-and-release-gate-completion.md` | adjacent proposal | terminal finalizer contract | 2026-04-15 | High | `state_12` still owns terminal receipt/report/run-state production. | Confirms `P045` keeps the handoff coherent. | Dependency contract source. |
| MAP-09 | `scripts/test-gate.sh` | verification | repository-owned proof inventory | 2026-04-15 | High | No `proposal-045` gate exists today, but the draft now explicitly owns adding it. | Confirms the old proof-lane blocker is stale. | Gate inventory seam. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | frozen delivery configuration | `control-plane/crates/domain/src/run.rs` | northbound -> persisted run -> executor | 2026-04-15 | High | Storage exists, but current daemon owner path cannot populate it yet. | Proposal must clearly own the input chain. | Main data seam. |
| DATA-02 | terminal receipt backfill | `Chainworks Forge/Engine/WorkflowOrchestrator.swift` | persisted artifacts -> release summary -> delivery_receipt | 2026-04-15 | High | Current Swift backfill depends on existing release summary, and the draft now explicitly mirrors that dependency. | Confirms the old backfill ambiguity is closed. | Former blocker seam. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | terminal `delivery_receipt` backfill eligibility | Proposal + Swift baseline | 2026-04-15 | High | Current draft now says terminal backfill requires `delivery_config`, `worktree_root`, and prior release-agent lineage / non-nil `currentReleaseResultSummary()`. | Old blocker is stale. | Cleared seam. |
| INT-02 | delivery-config northbound ownership | Proposal + current repo | 2026-04-15 | High | Current daemon lacks the input path, and the draft now correctly owns it. | Old blocker is stale. | Cleared seam. |
| INT-03 | release-task routing | Proposal + baseline + current repo | 2026-04-15 | High | `2f` now correctly splits the two release agents instead of collapsing them into one coordinator call. | Old blocker is stale. | Cleared seam. |
| INT-04 | preserve-vs-backfill handoff | Proposal + adjacent proposal + Swift baseline | 2026-04-15 | High | Current draft now explicitly says existing `delivery_receipt` is authoritative and `state_12` backfills only when absent. | Old blocker is stale. | Cleared seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Commit/push deterministic execution | Specified | DOC-01, MAP-01, MAP-07 | executor + git service parity | Stable and explicit. |
| Publish deterministic execution | Specified | DOC-01, MAP-01, MAP-07 | executor + publish service parity | Stable and explicit. |
| Frozen delivery-config happy path | Specified | DOC-01, MAP-02, MAP-03, MAP-04, MAP-05, MAP-06 | StartRun / Run persistence / GraphQL / MCP | Stable and explicit. |
| Release-attempt failure receipt truth | Specified | DOC-01, MAP-07 | git/publish failure owner chain | Structured failure truth is now aligned. |
| Preserve-vs-backfill into `state_12` | Specified | DOC-01, DOC-07, MAP-07, MAP-08, INT-01 | release execution + terminal finalizer | Stable and explicit. |
| Pre-release failure / no-release-attempt path | Specified | DOC-01, MAP-07, INT-01 | terminal fallback owner path | Stable and explicit under strict Swift parity. |
| Repository-owned proof path | Specified | DOC-01, DOC-06, MAP-09 | `test-gate.sh` / `test-gates.md` | Stable and explicit at the proposal layer. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none | deterministic release slice | direct implementation slice | implementation audit after landing | 2026-04-15 | High | No flag-specific rollout owner is proposed. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | none required for readiness review | proposal-readiness only | n/a | 2026-04-15 | High | No analytics blocker in this slice. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal-owned gate | deterministic release routing + receipt truth | current repo uses named gate governance; draft now correctly owns `proposal-045` | focused gate for input path, git/publish services, partial failure, receipt persistence, and native routing | 2026-04-15 | High | Old proof-lane blocker is stale. |
| TEST-02 | proposal text vs stable parity | terminal receipt backfill | draft now explicitly mirrors stable Swift backfill eligibility | no remaining proposal-first test-strategy gap | 2026-04-15 | High | Cleared seam. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | frozen delivery-config input | old blocker: daemon cannot receive config | current draft now explicitly owns the full input path | 2026-04-15 | High | Previous input-path blocker is stale. |
| REAL-02 | release-agent routing | old blocker: either release agent runs the whole coordinator | current draft now routes git and publish separately | 2026-04-15 | High | Previous split-routing blocker is stale. |
| REAL-03 | release-attempt failure truth | old blocker: non-happy release paths lost structured truth | current draft now explicitly preserves structured failure truth | 2026-04-15 | High | Previous failure-semantics blocker is stale. |
| REAL-04 | proof governance | old blocker: no `proposal-045` lane | current draft now explicitly defines `proposal-045` | 2026-04-15 | High | Previous proof-lane blocker is stale. |
| REAL-05 | preserve-vs-backfill handoff | old blocker: release path and finalizer both claimed `delivery_receipt` without a rule | current draft now explicitly defines preserve-vs-backfill behavior | 2026-04-15 | High | Previous handoff blocker is stale. |
| REAL-06 | terminal backfill owner chain | old blocker: config/worktree alone were insufficient for parity-safe backfill | current draft now explicitly requires prior release-agent lineage / non-nil `currentReleaseResultSummary()` | 2026-04-15 | High | Previous terminal-backfill blocker is stale. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-04, DOC-05 | Safety motivation is clear and grounded. |
| Scope boundaries | Specified | DOC-01, DOC-07 | Core scope is well bounded and aligned with the adjacent release slice. |
| Reusable baseline coverage | Specified | BASE-03, BASE-04, BASE-05, MAP-07 | Baseline coverage is sufficient and current. |
| Screen / surface definition | Deferred intentionally | NAV-01 | This is not a UI-first slice. |
| Navigation / entry points | Deferred intentionally | NAV-01, NAV-02 | Operator entry path is already baseline-owned. |
| State handling | Specified | NAV-02, NAV-03, INT-01 | Terminal fallback semantics are now explicitly locked. |
| Data / API contract | Specified | MAP-02, MAP-03, MAP-04, MAP-05, MAP-06, DATA-01 | Northbound/persistence/input contract is clearly owned now. |
| Persistence / caching | Specified | MAP-07, DATA-02, INT-01 | Receipt persistence and backfill rules are explicitly locked. |
| Permissions / auth expiry | Deferred intentionally | FLAG-01 | Not part of this proposal. |
| Feature flags / rollout / rollback | Deferred intentionally | FLAG-01 | Not a blocking concern in this slice. |
| Analytics / instrumentation | Deferred intentionally | METRIC-01 | Not required for readiness here. |
| Testing strategy | Specified | TEST-01, TEST-02 | Gate ownership is clearly specified. |
| Dependencies / integration points | Specified | DOC-07, NAV-03, INT-01 | `P044` dependency is clear and coherent. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P045` continues to preserve current Swift receipt/backfill semantics unless a later proposal explicitly replaces them.
- ASSUMP-02: `state_12` remains a preserve-or-backfill terminal owner, not a second independent receipt truth source.
- QUESTION-01: No blocking proposal-readiness question remains on this pass.
- BLOCKER-01: None.

## O. Research Triggers / External Questions
No external research trigger was required. Local proposal/docs/code/baseline evidence is sufficient for a proposal-readiness verdict.
