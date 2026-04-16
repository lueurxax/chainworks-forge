# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/047-session-reuse-and-cancellation-settlement.md` | 2026-04-15 | High | The current draft has already closed the stale review findings around budget mapping, execution-first cancellation settlement, and `FreshAfterInvalidation`, but it still under-specifies live ACP transport ownership and legacy schema migration. | Review could keep replaying stale blockers or miss the two live gaps. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should still anchor to the current stable host-system baseline before judging the daemon draft. | Review could miss already-stable owner contracts. | Intake baseline. |
| DOC-03 | `docs/reference/session-lineage-reuse-and-operator-reset.md` | 2026-04-15 | High | Stable session reuse is ownership-bounded, execution-first, and transport-backed; execution truth persists session provenance, but live reuse still depends on an actually reusable runtime session. | Proposal can sound aligned while still missing live transport ownership. | Main stable-reference anchor for reuse. |
| DOC-04 | `docs/reference/run-control.md` | 2026-04-15 | High | Stable cancellation truth is only settled after agent executions are terminal and runtime session close outcomes are recorded. | Proposal could mis-anchor cancellation or reuse cleanup. | Main stable-reference anchor for cancellation. |
| DOC-05 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-15 | High | Canonical persisted truth lives on `AgentExecution` / `StageExecution` before queue and reader fallbacks. | Proposal could still under-specify execution ownership. | Supporting stable-reference anchor. |
| DOC-06 | `docs/reference/acp-runtime-transport.md` | 2026-04-15 | High | Stable ACP runtime ownership is transport-shaped: session creation, prompt submission, and session close belong to a live runtime transport, not to a one-shot stateless wrapper. | Proposal can still under-port the current transport lifetime model. | Main stable-reference anchor for transport ownership. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake baseline | 2026-04-15 | High | Fresh enough as an intake artifact. | Entry baseline. |
| BASE-02 | `docs/reference/session-lineage-reuse-and-operator-reset.md` | Partially refreshed | stable session reuse and execution-side provenance contract | 2026-04-15 | High | Refreshed narrowly against the current proposal and current ACP/Rust code. | Reuse parity baseline. |
| BASE-03 | `docs/reference/run-control.md` | Partially refreshed | stable cancellation settlement contract | 2026-04-15 | High | Refreshed narrowly against the current proposal’s cancellation phase text and run-reader split. | Cancellation baseline. |
| BASE-04 | `docs/reference/execution-truth-and-recovery.md` | Partially refreshed | stable execution-first ownership | 2026-04-15 | High | Refreshed narrowly against the proposal’s execution provenance and cancellation truth shape. | Execution-truth baseline. |
| BASE-05 | `docs/reference/acp-runtime-transport.md` | Partially refreshed | stable transport/session ownership | 2026-04-15 | High | Refreshed narrowly against current Rust ACP manager/adapter/transport topology. | Transport baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - durable ACP session lineage in Rust
  - generation-scoped context budget
  - two-phase cancellation settlement
  - northbound run-reader wiring for cancellation settlement visibility
- Out of scope:
  - implementation audit or gate execution
  - thin-client UI work
  - unrelated delivery / approval proposals
- Assumptions:
  - review mode is `proposal-readiness`
  - the current draft should be judged against the latest working-tree text, not the stale prior review basis
  - stable session-lineage, run-control, execution-truth, and ACP transport references remain the parity authority
- Blockers:
  - live ACP session reuse still lacks a transport-lifetime owner in the proposal
  - migration strategy for the already-existing `session_lineages` table is still missing

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | report / recovery / execution readers | Baseline + current repo + proposal | 2026-04-15 | High | The draft now correctly keeps session provenance and cancellation truth execution-first. | Review could keep a stale cancellation-owner finding open. | Execution / recovery seam. |
| NAV-02 | run single-item vs list readers | Baseline + current repo + proposal | 2026-04-15 | High | Current GraphQL already splits canonical single-run reads from projection-backed lists, while MCP `runs.list` still does not. The proposal targets a real current seam. | Review could miss whether the proposal is grounded in current northbound topology. | Reader-boundary seam. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/acp/src/manager.rs`, `control-plane/crates/acp/src/adapters/claude.rs`, `control-plane/crates/acp/src/transport.rs` | Rust ACP runtime | current ACP session lifecycle owner | 2026-04-15 | High | Current Rust ACP execution is one-shot: manager only dispatches, adapters spawn a fresh subprocess per invoke, and `run_acp_session` always closes the session and tears down the process. | Proposal can still claim session reuse while naming too little runtime ownership. | Transport blocker. |
| MAP-02 | `Chainworks Forge/Engine/ACPAdapters/ClaudeAgentACPTransport.swift`, `Chainworks Forge/Engine/RuntimeSessionBridge.swift` | Swift stable transport | implemented live-session owner path | 2026-04-15 | High | Stable reuse keeps live ACP subprocess/session handles in transport-owned `activeSessions`, and `executeInExistingSession(...)` submits a new prompt onto that existing live session. | Proposal can still under-port the live transport owner chain. | Transport parity proof. |
| MAP-03 | `control-plane/crates/db/migrations/002_projections.sql` | Rust DB | existing legacy session-lineage schema | 2026-04-15 | High | Current DB already defines `session_lineages` with a legacy shape (`stage_id`, `lineage_kind`, `previous_session_id`). | Proposal can still describe an impossible greenfield migration. | Migration blocker. |
| MAP-04 | `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/graphql-server/src/types/run.rs`, `control-plane/crates/mcp-server/src/tools/runs.rs`, `control-plane/crates/db/src/repos/projections.rs`, `control-plane/crates/db/src/repos/runs.rs` | Rust northbound readers | current run read surfaces | 2026-04-15 | High | GraphQL `run(id)` is canonical and `runs` is projection-backed already; MCP `runs.get` is canonical while `runs.list` still returns canonical rows instead of projection summaries. | Proposal can still be judged against the wrong current northbound baseline. | Reader-surface grounding. |
| MAP-05 | `control-plane/crates/workflow/src/catalog.rs` | Rust workflow parser | current catalog reuse-field ingestion | 2026-04-15 | High | Current Rust catalog already parses `session_reuse_scope` and `session_family_id`, so the proposal’s compiler/plan work lands on a real current seam instead of an invented one. | Review could overstate parser-side missingness. | Freshness proof. |
| MAP-06 | `Chainworks Forge/Engine/ContextBudgetGuard.swift`, `Chainworks Forge/Models/AgentSessionLineage.swift`, `Chainworks Forge/Engine/SessionReusePolicy.swift`, `Chainworks Forge/Models/Run.swift`, `Chainworks Forge/Engine/RunCancellationCoordinator.swift` | Swift stable owner chain | stable parity source | 2026-04-15 | High | The current draft now matches the stable budget mapping, invalidation taxonomy, and execution-first cancellation settlement. | Review could keep stale red findings alive after the draft already fixed them. | Stale-review correction proof. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | live ACP session reuse ownership | Baseline + current repo + proposal | 2026-04-15 | High | Stable reuse depends on a transport-owned live session; current Rust ACP code has no equivalent persistent owner yet. | Proposal still under-specifies the transport lifetime boundary. | High-severity blocker. |
| INT-02 | legacy `session_lineages` schema migration | Current repo + proposal | 2026-04-15 | High | Current DB already contains a `session_lineages` table with incompatible columns. | Proposal still reads as greenfield schema creation without an upgrade path. | High-severity blocker. |
| INT-03 | run single-item vs list reader split | Baseline + current repo + proposal | 2026-04-15 | High | Current GraphQL and MCP run readers are split across canonical and projection paths exactly where the proposal says they are. | No live blocker remains on this reader split; this part of the draft is grounded. | Fresh grounding. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Execution-side session provenance on `AgentExecution` | Specified | DOC-01, DOC-03, MAP-06 | proposal §2d-ii + stable session reference | Old execution-side provenance blocker is closed. |
| Stable budget decision mapping | Specified | DOC-01, DOC-03, MAP-06 | proposal §2e + stable `ContextBudgetGuard` | Old budget blocker is closed. |
| Execution-first cancellation settlement truth | Specified | DOC-01, DOC-04, DOC-05, MAP-06 | proposal §2f + stable run-control owner chain | Old cancellation-owner blocker is closed. |
| Generic invalidation reuse disposition | Specified | DOC-01, DOC-03, MAP-06 | proposal disposition enum/policy vs stable enum/policy | Old taxonomy blocker is closed. |
| Live ACP session ownership across invocations | Contradicted by repo | DOC-01, DOC-06, MAP-01, MAP-02, INT-01 | proposal §2d vs current ACP manager/adapter/transport | Proposal still lacks the runtime owner path needed for actual session reuse. |
| Legacy `session_lineages` migration strategy | Contradicted by repo | DOC-01, MAP-03, INT-02 | proposal §2a / file list vs current DB migration `002` | Proposal still has no executable migration path for existing installs. |
| Single-run vs list cancellation readers | Specified | DOC-01, DOC-04, MAP-04, INT-03 | GraphQL/MCP run read surfaces | The draft is grounded correctly here. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal acceptance | lineage / reuse semantics | current AC proves same-session reuse, budget mapping, and disposition paths | add proof that live reuse works on the real Rust transport owner path, not just via persisted `sessionId` strings | 2026-04-15 | High | Transport-lifetime owner is still missing. |
| TEST-02 | proposal acceptance | DB migration safety | current AC proves lineage/generation/event population | add proof that migration applies on a DB already containing legacy `session_lineages` from `002_projections.sql` | 2026-04-15 | High | Schema upgrade path is still missing. |
| TEST-03 | proposal acceptance | northbound run readers | current AC now correctly distinguishes canonical single-run vs projection list surfaces | no blocking gap remains here; keep summary-only list proof explicit | 2026-04-15 | High | Reader split is already specified well enough. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | ACP reuse transport | proposal says reused executions can issue `session/prompt` on the existing `sessionId`, and transport only needs to skip `session/new` + `initialize` | current Rust adapters spawn a fresh subprocess per invoke, `run_acp_session` always closes the session, and manager owns no live session registry | 2026-04-15 | High | One high-severity transport-owner blocker remains. |
| REAL-02 | lineage migration | proposal describes `session_lineages` as a new canonical table in `006_session_lineage.sql` | current DB already created `session_lineages` in `002_projections.sql` with incompatible columns | 2026-04-15 | High | One high-severity migration blocker remains. |
| REAL-03 | old review basis | old review said budget mapping, execution-first cancellation settlement, and generic invalidation were still missing | current proposal text already contains those fixes | 2026-04-15 | High | Prior red review findings are stale and should be retired. |
| REAL-04 | run-reader split | proposal says single-run readers should stay canonical while list readers project only summary settlement info | current GraphQL already behaves that way; MCP `runs.list` still does not | 2026-04-15 | High | This proposal surface is grounded in a live current seam, not stale speculation. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-03, DOC-04, DOC-06 | The daemon seams are real and correctly scoped. |
| Scope boundaries | Specified | DOC-01 | The draft stays focused on session reuse, budget, and cancellation settlement. |
| Reusable baseline coverage | Partial | BASE-02, BASE-03, BASE-04, BASE-05, REAL-01, REAL-02 | Most stale gaps are fixed, but two implementation-blocking parity gaps remain. |
| Data / execution contract | Partial | MAP-01, MAP-02, MAP-03, REAL-01, REAL-02 | Persistence design is mostly specified, but live transport lifetime and schema migration are still underspecified. |
| Failure / recovery semantics | Specified | DOC-03, DOC-04, DOC-05, MAP-06 | Execution-first cancellation and reuse taxonomy are now aligned. |
| Reader / operator semantics | Specified | DOC-01, DOC-04, MAP-04, REAL-04 | Single-run/list split is described concretely enough. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Acceptance now covers the stale findings, but not the two live blockers. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P047` still intends parity with the stable Swift session-reuse, run-control, and ACP transport slices, not a transport model fork.
- ASSUMP-02: `session_lineages` from `002_projections.sql` is a real upgrade input, not disposable noise that implementation can ignore silently.
- QUESTION-01: Should the Rust live-session registry live in `AcpRuntimeManager`, in provider adapters, or in a dedicated `acp/src/session.rs` ownership layer?
- QUESTION-02: Should legacy `session_lineages` rows be migrated forward, renamed aside, or explicitly discarded with a documented compatibility note?
- BLOCKER-01: the proposal still under-specifies the owner for live ACP sessions across invocations.
- BLOCKER-02: the proposal still has no migration strategy for the already-existing `session_lineages` table.

## L. Research Triggers / External Questions
No external research trigger is required. Local proposal, baseline, and current-code evidence are sufficient for a readiness verdict.
