# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/025-per-agent-mcp-policy-and-runtime-validation.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/025-mcp-policy-review-notes.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/025-mcp-policy-review-notes-v2.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/goose-server-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/execution-truth-and-recovery.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - provider-platform ownership
  - live runtime / Goose control-plane boundary
  - execution-truth owner split
  - operator-shell report/comparison ownership
- Baseline refreshed:
  - current catalog permission shape
  - current Goose session bootstrap and blanket extension removal path
  - current preflight authority
  - current run-owned KPI/report lane
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseTransport.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseServerTransport.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/PreflightService.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift`
- Targeted context refresh performed: `Yes`
- External research used: `None`
- Research pack: `None`
- Sources reused: repo-local baselines and stable refs only
- Sources refreshed: current catalog schema, Goose transport/session seams, preflight ownership, report/comparison KPI consumers
- Time-sensitive external guidance: `None`
- Code areas inspected:
  - catalog schema and permission profile model
  - Goose transport/session request/response contract
  - Goose session bootstrap and extension cleanup path
  - preflight authority
  - frozen `Run` and per-attempt `AgentExecution` persistence owners
  - shell-owned report/comparison KPI lanes
- Current repo contradictions found:
  - none live after the current proposal edits
  - the last scope / acceptance wording blocker is now closed by the updated `repo-owned MCP server registry` language plus AC-2’s explicit separation from machine-local runtime capability truth
- Runtime evidence used: `None`
- Provenance of key evidence: `/Users/user/Documents/Chainworks Forge/docs/reviews/025-per-agent-mcp-policy-and-runtime-validation-evidence-pack.md`
- Remaining assumptions:
  - Goose-backed runtime remains the first implementation target
  - current report/comparison surfaces remain the operator shell spine for post-run MCP truth
- Remaining blockers: `None`

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top risks:
  1. No live proposal-text blockers remain; the main remaining risk is ordinary implementation discipline when wiring DSL, Goose reconciliation, preflight, persistence, and shell-owned reporting together.
  2. The draft still has no dedicated proof-owner section, so implementation proof expectations remain implicit rather than explicit.
  3. Conceptual-name cleanup remains a real migration burden even after the contract is fixed.
- Top opportunities:
  1. The proposal is grounded in a real current-head seam: Goose sessions still start with enabled extensions and are then blanket-cleaned.
  2. The existing control-plane/runtime split gives the proposal a clear path if it adopts the repo’s current owner boundaries instead of inventing new ones.
  3. The current shell-owned report/comparison lanes can absorb requested/effective MCP truth and telemetry without a parallel metrics or diagnostics subsystem.

## 2. Proposal Scope and Completeness
- In scope:
  - explicit per-agent MCP policy
  - explicit server mapping / runtime reconciliation
  - preflight/runtime validation
  - requested-vs-effective diagnostics
  - burn telemetry
- Out of scope:
  - interactive MCP editor UI
  - generic extension marketplace
  - non-Goose runtime execution beyond capability hooks
  - build/run proof
- Deferred intentionally:
  - broader provider/model validation already covered by stable refs
  - non-Goose execution implementation
- Most important baseline refreshes performed:
  - verified current catalog still exposes only coarse `permission_profiles.*.mcp.allow`
  - verified current Goose transport still removes all enabled extensions after session start
  - verified current preflight is prelaunch-only and does not own post-session truth
  - verified current report/comparison surfaces already consume run-owned KPI/report JSON
- Most important contradictions with current repo:
  - none remain live in the current proposal text
- Most important missing or partial states:
  - no proposal-blocking text gaps remain
  - a dedicated proof-owner section is still optional future hygiene, not a readiness blocker

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI findings in the current reread.

### 5.2 UX Findings
- No live UX findings in the current reread.

### 5.3 iOS Architecture Findings
- No live architecture findings in the current reread.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: earlier rounds had a mismatch between top-level wording and the later owner split.
  Tradeoff: short summary wording was convenient, but it had reopened authority ambiguity.
  Decision: the current draft now keeps scope, body, and acceptance aligned on repo-owned mapping versus machine-local runtime capability truth.
  Owner: Sections 3, 5.1, 7, 9.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Implement the proposal with the now-explicit owner split preserved across DSL, preflight, execution persistence, and shell-owned reporting | iOS Architecture | Implementation | Next implementation pass | None | Requested, predicted, and actual MCP truth never collapse back into one layer | Proposal text aligned |
| P2 | Add an explicit proof/test-owner section if future reviewers want tighter handoff hygiene | iOS Architecture | Proposal text | Optional future cleanup | None | Implementation audit can point to a dedicated proving lane | Optional completeness hygiene |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Owner alignment | Repo mapping, machine-local capability, preflight prediction, runtime settlement, and KPI/report ownership stay separate in implementation | DSL, preflight, execution truth, and report readers all preserve the declared split | No layer silently widens or reconstructs MCP truth from weaker evidence | Implementation audit | Hold if implementation re-merges any of the now-separated truths |
| MCP proof expectations | Implementation audit proves preflight, reconciliation, persistence, and reporting on the current tree | Focused and full-regression proof both read the declared owner boundaries | No success claim depends on ad hoc proof | Implementation audit | Hold if proof cannot show requested/predicted/actual separation end to end |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No blocking evidence gaps remain for proposal-readiness. Current repo-local docs and code paths are enough to judge the proposal text.

### Open Questions
- `QUESTION-01`: Should the draft add a lightweight proof-owner section now, or leave that to implementation audit discipline later?
