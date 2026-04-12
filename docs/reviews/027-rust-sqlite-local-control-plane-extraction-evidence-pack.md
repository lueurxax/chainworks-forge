# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md` | 2026-04-11 | High | Current draft now explicitly separates parity-phase ownership from post-cutover ownership: `§8.2` keeps the client canonical through `P027`, names `ExecutionService`, `RecoveryCoordinator`, `RunReportBuilder`, and `WorkflowMapProjectionService` as app-owned during parity, and defers authority transfer to `P031`. | Review would be stale if it continued to treat the old owner-collapse wording as live. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-11 | Medium | Intake baseline exists, but stable reference docs still matter more for the affected surfaces. | Review could over-trust intake context without targeted stable-doc refresh. | Intake baseline. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-11 | High | Current stable baseline says the product already has a stable operator shell and stable execution/recovery/report behavior. | Proposal must still be reviewed as delta over a real current baseline. | Stable baseline authority. |
| DOC-04 | `docs/reference/workflow-execution-engine.md` | 2026-04-11 | High | Current engine truth remains app-owned on `HEAD`. | Needed to verify whether the new parity-owner language actually matches current repo reality. | Stable engine authority. |
| DOC-05 | `docs/reference/live-provider-execution-slice.md` | 2026-04-11 | High | Current stable runtime slice still says the app remains the control plane. | Needed to verify that the revised parity-owner language closes, rather than conflicts with, current runtime truth. | Stable runtime/control-plane authority. |
| DOC-06 | `docs/reference/operator-experience.md` | 2026-04-11 | High | Current operator spine still owns Runs Home, reports, recovery, workflow map, and artifact inspection as app surfaces. | Needed to verify that the new `P027` text keeps those readers app-owned during parity. | Stable operator-shell authority. |
| DOC-07 | `docs/proposals/029-mcp-northbound-control-plane-server.md` | 2026-04-11 | High | `P029` explicitly owns the MCP northbound command plane after parity exists. | Needed to check whether `P027` still over-scopes MCP exposure. | Follow-on dependency. |
| DOC-08 | `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md` | 2026-04-11 | High | `P031` explicitly owns the first user-visible ownership transfer. | Needed to verify that `P027` now truly defers client cutover to `P031`. | Follow-on dependency. |
| DOC-09 | `docs/proposals/043-query-projections-and-client-consumption-contract.md` | 2026-04-11 | High | `P043` explicitly owns the GraphQL read-model and client-consumption contract for the thin client. | Needed to verify that `P027` now demotes client migration from its own acceptance bar. | Follow-on dependency. |
| DOC-10 | current `027` review/evidence artifacts | 2026-04-11 | High | Previous local artifacts are now stale red-basis comparators only. | Review could accidentally repeat already-closed blockers. | Freshness comparator. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Partially refreshed | intake baseline | 2026-04-11 | Medium | Useful only as entry point; stable docs were needed for the affected surfaces. | Intake baseline. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current product boundary | 2026-04-11 | High | Fresh for this slice. | Stable baseline. |
| BASE-03 | `docs/reference/workflow-execution-engine.md` | Reused | current control-plane owners | 2026-04-11 | High | Fresh and directly relevant. | Stable engine boundary. |
| BASE-04 | `docs/reference/live-provider-execution-slice.md` | Reused | current live control-plane boundary | 2026-04-11 | High | Fresh and directly relevant. | Stable runtime/control-plane boundary. |
| BASE-05 | `docs/reference/operator-experience.md` | Reused | current operator shell owners | 2026-04-11 | High | Fresh and directly relevant. | Stable shell boundary. |
| BASE-06 | proposal-specific integration context | Missing | none | 2026-04-11 | High | No dedicated integration-context artifact exists; targeted code refresh was sufficient. | Not blocking. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - Rust daemon parity replica
  - SQLite durability model
  - product-owned workflow engine inside the daemon
  - command journal, work queue, restart repair
  - parity readiness for later validation and cutover
- Out of scope:
  - thin-client cutover
  - finalized GraphQL client contract
  - finalized MCP command exposure
  - distributed orchestration
  - remote hosting topology
- Deferred intentionally:
  - parity harness specifics
  - daemon lifecycle productization
  - full thin-client query/command contract
- Assumptions:
  - `P027` remains parity-first and intentionally defers user-visible ownership transfer to `P031`
- Open questions:
  - none blocking for proposal-readiness
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `RunsHomeView` | Baseline + current repo | 2026-04-11 | High | Current operator landing surface still reads `Run` rows directly through SwiftData and `ExecutionService`. | Needed to verify parity-owner wording against the live shell owner. | Current shell owner. |
| NAV-02 | `RunReportView` | Baseline + current repo | 2026-04-11 | High | Current report surface is still a SwiftUI/SwiftData shell over persisted artifacts and report metadata. | Needed to verify that `P027` now leaves report truth app-owned during parity. | Current report owner. |
| NAV-03 | `RecoverySheet` / recovery surfaces | Baseline + current repo | 2026-04-11 | High | Current recovery next-action truth is still app-owned and tied to `RecoveryCoordinator`. | Needed to verify that `P027` now leaves recovery truth app-owned during parity. | Current recovery owner. |
| NAV-04 | workflow map / run detail | Baseline + current repo | 2026-04-11 | High | Current run-detail topology projection is produced in-app through `WorkflowMapProjectionService`. | Needed to verify that `P027` now leaves workflow-map projection app-owned during parity. | Current workflow-map owner. |
| NAV-05 | `IdeaListView` operator summary | Current repo | 2026-04-11 | High | Current idea-owned summary and active-run context also read through SwiftData and `ExecutionService`. | Confirms broader shell is still app-owned on `HEAD`. | Current shell adjacency. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/ExecutionService.swift` | app control plane | app-scoped run/orchestrator owner | 2026-04-11 | High | `ExecutionService` is still the app-scoped owner for starting runs, resuming runs, approvals, and maintenance. | Confirms the proposal’s new parity-owner wording is grounded in current reality. | Current control-plane owner. |
| MAP-02 | `Chainworks Forge/Engine/RecoveryCoordinator.swift` | recovery | current next-action / retry / clone owner | 2026-04-11 | High | Recovery legality and report emission are still enforced in-app. | Confirms the proposal’s parity-owner wording is grounded in current reality. | Current recovery owner. |
| MAP-03 | `Chainworks Forge/Engine/RunReportBuilder.swift` | reporting | immutable/latest report truth owner | 2026-04-11 | High | Report emission and latest-summary mutation are still in-app and tied to SwiftData/artifact writes. | Confirms the proposal’s parity-owner wording is grounded in current reality. | Current report owner. |
| MAP-04 | `Chainworks Forge/Engine/WorkflowMapProjectionService.swift` | read model | current in-app workflow-topology projection | 2026-04-11 | High | Current run-detail topology projection is computed in-app from SwiftData snapshots plus live orchestrator state. | Confirms the proposal’s parity-owner wording is grounded in current reality. | Current workflow-map owner. |
| MAP-05 | `Chainworks Forge/Views/RunsHomeView.swift` | shell UI | operator landing screen | 2026-04-11 | High | Current Runs Home is a direct `@Query` + `ExecutionService` consumer, not a GraphQL client. | Confirms that client migration is still future-state, not current `HEAD`. | Current shell anchor. |
| MAP-06 | `Chainworks Forge/Views/RunReportView.swift` | shell UI | report/detail screen | 2026-04-11 | High | Current report view is a direct SwiftData consumer with local state orchestration. | Confirms that report migration is still future-state, not current `HEAD`. | Current shell anchor. |
| MAP-07 | `Chainworks Forge/Views/IdeaListView.swift` | shell UI | idea/operator summary surface | 2026-04-11 | High | Current idea list also reads SwiftData and `ExecutionService` directly. | Confirms that broader shell migration is still future-state, not current `HEAD`. | Current shell anchor. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | parity-vs-authority contract | `P027 §3.4`, `§8.2`, `§12.3`, `§13` | proposal owner model | 2026-04-11 | High | The current draft now explicitly keeps the client canonical through `P027`, marks daemon projections as shadow truth, and defers authority transfer to `P031`. | This is the main closure for the old red-basis blocker. | Closed owner seam. |
| DATA-02 | GraphQL and MCP northbound scope | `P027`, `P029`, `P031`, `P043` | roadmap ownership | 2026-04-11 | High | The current draft now demotes client migration and client command routing to `P031`, `P043`, and `P029` in `§11` and `AC-3/4/7`, while keeping GraphQL/MCP only as preserved boundary shape. | This is the main closure for the old northbound-scope blocker. | Closed scope seam. |
| DATA-03 | current control-plane and read-model path | stable refs + current code | current repo reality | 2026-04-11 | High | Current app still owns control-plane logic and read-model consumers. | Needed to validate that the new proposal wording matches, rather than contradicts, current repo reality. | Current-owner anchor. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | app-owned execution control plane | Stable refs + current repo | 2026-04-11 | High | The app still owns run lifecycle, transitions, approvals, and runtime-facing orchestration. | Current draft now explicitly preserves that fact through parity. | Main owner seam. |
| INT-02 | operator shell read-models | Stable refs + current repo | 2026-04-11 | High | Runs Home, reports, recovery, workflow map, and idea summaries still read through current local owners. | Current draft now explicitly preserves that fact through parity. | Main shell seam. |
| INT-03 | northbound MCP command plane | Proposal dependency | 2026-04-11 | High | `P029` owns command/control MCP exposure after parity exists. | Current draft now defers client command routing accordingly. | Follow-on seam. |
| INT-04 | GraphQL read-model contract | Proposal dependency | 2026-04-11 | High | `P043` owns required read surfaces, client inference prohibitions, and GraphQL query contract. | Current draft now defers client-side migration accordingly. | Follow-on seam. |
| INT-05 | thin-client ownership transfer | Proposal dependency | 2026-04-11 | High | `P031` explicitly owns the first user-visible transfer from client-owned logic to a thin UI. | Current draft now defers authority transfer accordingly. | Follow-on seam. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `DOC-04` | app-owned start-run path and daemon parity goal | Entry problem statement is clear. |
| Happy path | Specified | `DOC-01` | parity replica + SQLite truth | Core daemon shape is clear enough. |
| Loading | Deferred intentionally | `DOC-01` | not a proposal-readiness blocker here | Acceptable. |
| Empty | Deferred intentionally | `DOC-01` | not central to this extraction proposal | Acceptable. |
| Validation error | Specified | `DOC-01`, `MAP-02`, `MAP-03` | current app-side report/recovery truth | Reader ownership during parity is now explicit. |
| Backend error | Specified | `DOC-01`, `MAP-01` | app-owned control-plane today | Parity-owner language now explicitly matches current repo reality. |
| Offline / degraded | Deferred intentionally | `DOC-01` | out of scope for local-first daemon extraction | Acceptable. |
| Retry / recovery | Specified | `DOC-01`, `DOC-06`, `MAP-02` | current app-owned recovery coordinator | Proposal now keeps current recovery owner live through parity. |
| Auth / permission expiry | Deferred intentionally | `DOC-01` | outside current proposal slice | Acceptable. |
| Rollback / cancellation | Specified | `DOC-01`, `MAP-01` | current app-owned cancellation path | Parity-owner wording is clear enough. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | parity validation / future feature-flagged slices | parity extraction and later cutover | parity proof before cutover, no shared ownership window in `P027` | authority transfer deferred to `P031` | 2026-04-11 | Medium | Adequate for proposal-readiness. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | parity / golden-run validation | prove daemon behavior matches client behavior | future `P041` parity harness | 2026-04-11 | Medium | Implementation-side proof remains future work, but proposal scope is now clear. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | stable reference + roadmap | parity readiness | `P027` names parity validation conceptually | `P041` to own golden runs and behavioral diff | 2026-04-11 | Medium | Consistent with the updated parity-first scope. |
| TEST-02 | roadmap | northbound MCP | `P029` owns command-plane verification later | client command routing stays out of `P027` acceptance | 2026-04-11 | High | No live proposal-text blocker remains here. |
| TEST-03 | roadmap | GraphQL read contract / client consumption | `P043` owns read-surface contract later | client-side migration stays out of `P027` acceptance | 2026-04-11 | High | No live proposal-text blocker remains here. |
| TEST-04 | roadmap | user-visible thin-client cutover | `P031` owns first visible ownership transfer later | authority transfer stays out of `P027` acceptance | 2026-04-11 | High | No live proposal-text blocker remains here. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | ownership boundary | `P027` now says the client remains canonical during parity and authority transfer is deferred to `P031` | current stable refs and code still keep the app as the control plane and shell owner | 2026-04-11 | High | Proposal and current repo reality are now aligned. |
| REAL-02 | northbound boundary | `P027` now treats GraphQL/MCP as preserved target boundary shape while deferring client migration and client routing to follow-on proposals | current `HEAD` UI still reads through `SwiftData + ExecutionService` | 2026-04-11 | High | Proposal and current repo reality are now aligned. |
| REAL-03 | operator shell readers | `P027` now explicitly names current app-owned readers that remain canonical through parity | current `RunsHomeView`, `RunReportView`, `IdeaListView`, and `WorkflowMapProjectionService` are still local `SwiftData + ExecutionService` consumers | 2026-04-11 | High | Proposal and current repo reality are now aligned. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01` | Problem statement is clear and grounded. |
| Scope boundaries | Specified | `DOC-01`, `DATA-01`, `DATA-02` | Parity-only scope is now explicit and consistent. |
| Reusable baseline coverage | Partially refreshed | `BASE-01` to `BASE-05` | Stable docs were sufficient after targeted refresh. |
| Screen / surface definition | Specified | `DOC-06`, `NAV-01` to `NAV-05` | Current shell owners and parity behavior are explicit. |
| Navigation / entry points | Specified | `NAV-01` to `NAV-05` | Current shell entry points are known and parity ownership is explicit. |
| State handling | Specified | `H` matrix | No live proposal-text gap remains. |
| Data / API contract | Specified | `DATA-01`, `DATA-02`, `REAL-01`, `REAL-02` | Owner split and northbound sequencing are now explicit. |
| Persistence / caching | Specified | `DOC-01` | SQLite and daemon persistence shape is adequately specified at proposal level. |
| Permissions / auth expiry | Deferred intentionally | `DOC-01` | Out of scope here. |
| Feature flags / rollout / rollback | Partial | `FLAG-01` | Adequate for proposal-readiness; implementation can refine later. |
| Analytics / instrumentation | Partial | `METRIC-01` | Parity proof remains future work by design. |
| Testing strategy | Specified | `TEST-01` to `TEST-04` | Roadmap ownership is now clear and non-conflicting. |
| Dependencies / integration points | Specified | `DOC-07`, `DOC-08`, `DOC-09`, `INT-03` to `INT-05` | Current draft now aligns with follow-on proposal ownership. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Future edits will preserve the now-explicit parity-vs-cutover owner split.
- QUESTION-01: No open proposal-text question remains that blocks readiness.
- BLOCKER-01: None.

## O. Research Triggers / External Questions
No external research trigger was required for this pass. Local proposal/docs/code/baseline evidence was sufficient for a proposal-first verdict.
