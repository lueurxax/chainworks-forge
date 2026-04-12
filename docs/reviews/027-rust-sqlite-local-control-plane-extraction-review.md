# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/027-rust-sqlite-local-control-plane-extraction.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/live-provider-execution-slice.md`
  - `docs/reference/operator-experience.md`
  - `docs/proposals/029-mcp-northbound-control-plane-server.md`
  - `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md`
  - `docs/proposals/043-query-projections-and-client-consumption-contract.md`
  - current `027` review/evidence artifacts as stale-basis comparators
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/live-provider-execution-slice.md`
  - `docs/reference/operator-experience.md`
- Baseline reused:
  - stable current-head reference docs above
- Baseline refreshed:
  - targeted reread of `P027 §3.4`, `§4`, `§5.1`, `§8`, `§11`, `§12.3`, and `§13`
  - targeted follow-on dependency reread for `P029`, `P031`, and `P043`
  - targeted current-head code refresh for the app-owned control-plane and operator-shell readers
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- Targeted context refresh performed:
  - `ExecutionService`
  - `RecoveryCoordinator`
  - `RunReportBuilder`
  - `WorkflowMapProjectionService`
  - `RunsHomeView`
  - `RunReportView`
  - `IdeaListView`
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
- Current repo contradictions found:
  - none that remain blocking after the latest draft changes
- Runtime evidence used: `None`
- Provenance of key evidence:
  - stable current-head reference docs
  - current proposal text
  - targeted current code-path mapping for the app-owned execution shell
- Remaining assumptions:
  - `P027` remains parity-first and intentionally defers user-visible ownership transfer to `P031`
- Remaining blockers:
  - none

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top risks:
  1. Remaining risk is implementation-side, not proposal-text: parity validation still has to prove the daemon can hold the same truth as the live app path.
  2. The draft now depends on follow-on proposals staying aligned on the same GraphQL/MCP split.
  3. The current system-shape diagram still points at the eventual thin-client target, so future edits should preserve the parity-vs-cutover distinction that is now explicit elsewhere.
- Top opportunities:
  1. The draft now has a clean parity-owner contract that future proposals can depend on directly.
  2. Acceptance criteria now separate daemon capability proof from client migration, which should make implementation slicing much safer.
  3. The split between daemon shadow truth in `P027` and canonical client cutover in `P031` is now reusable roadmap language for adjacent proposals.

## 2. Proposal Scope and Completeness
- In scope:
  - local Rust daemon as control-plane parity replica
  - SQLite durable local persistence
  - product-owned orchestration engine
  - command journal / work queue / restart repair
  - parity validation readiness for later cutover
- Out of scope:
  - user-visible thin-client cutover
  - finalized GraphQL client contract
  - finalized MCP command exposure
  - distributed orchestration guarantees
  - remote-host deployment topology
- Deferred intentionally:
  - parity harness specifics
  - daemon lifecycle productization
  - full thin-client query/command contract
- Most important baseline refreshes performed:
  - current app-owned control-plane boundary
  - current operator-shell readers
  - follow-on proposal ownership for MCP, GraphQL, and thin-client cutover
- Most important contradictions with current repo:
  - none still live at `proposal-text` level
- Most important missing or partial states:
  - none blocking for proposal-readiness

## 3. Proposal Readiness Verdict
- `Readiness = Green`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is **not** an Evidence Gap Review. Local proposal/docs/code/baseline evidence is sufficient for a proposal-first verdict.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` findings.

### 5.2 UX Findings
- No live UX `proposal-text` findings.

### 5.3 iOS Architecture Findings
- No live iOS architecture `proposal-text` findings.

## 6. Cross-Discipline Conflicts and Decisions
- No live cross-discipline conflict remains after the latest draft update.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Keep `P029`, `P031`, and `P043` aligned with the parity-owner split now stated in `P027` | iOS Architecture | proposal author | Before implementation handoff | roadmap consistency | no later draft reintroduces shared ownership during `P027` | none |
| P2 | Preserve the explicit statement that daemon projections are shadow truth during `P027` | iOS Architecture | proposal author | During future edits | `P027 §8.2`, `§12.3`, `§13` | no later edit silently reintroduces client migration into `P027` | none |
| P2 | Carry the same parity-vs-cutover wording into implementation plans and audits | iOS Architecture | implementation owner | Before implementation | implementation planning docs | implementation slices do not treat GraphQL/MCP client migration as part of `P027` itself | none |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Parity owner contract | whether implementation and follow-on docs preserve the same owner split | daemon treated as validated shadow truth, app remains canonical during `P027` | no shared ownership window is reintroduced | next proposal/audit round | hold if any artifact again says the client already stopped owning workflow decisions during `P027` |
| Northbound boundary sequencing | whether GraphQL/MCP remain target-boundary shape rather than `P027` completion gates | follow-on docs continue to own client migration and external command contracts | no `P027` acceptance or implementation plan absorbs `P029` / `P031` / `P043` scope | next proposal/audit round | hold if client migration is again presented as part of `P027` completion |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: No open proposal-text question remains that blocks readiness.

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
