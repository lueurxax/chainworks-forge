# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-first-runtime-transport-and-goose-decoupling.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/026-acp-runtime-plan-additive-profiles.md`
  - `/Users/user/Documents/Chainworks Forge/docs/evidence/codex-acp-research.md`
  - `/Users/user/Documents/Chainworks Forge/docs/evidence/acp-runtime-candidate-comparison.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/026-acp-first-runtime-transport-and-goose-decoupling-review.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/026-acp-first-runtime-transport-and-goose-decoupling-evidence-pack.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - provider-platform machine-local ownership
  - frozen-vs-runtime execution-truth ownership
  - shell-owned report / comparison / recovery spine
  - current Goose-backed live execution posture
- Baseline refreshed:
  - depended-on additive runtime-profile note
  - current catalog / provider / run-start / per-attempt owner seams in code
  - current ACP candidate-comparison evidence after adding `codex-acp`
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: `None present`
- Targeted context refresh performed: `Yes`
- External research used: `None in this pass`
- Research pack: `None`
- Sources reused: prior repo-local review/evidence artifacts plus stable baseline docs
- Sources refreshed:
  - current `P026` text
  - current `026-acp-runtime-plan-additive-profiles.md`
  - current `codex-acp-research.md`
  - current `acp-runtime-candidate-comparison.md`
  - current owner-boundary code seams
- Time-sensitive external guidance: `None`
- Code areas inspected:
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/Providers/ConfiguredProvider.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/GooseTransport.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Current repo contradictions found:
  - none live in the current reread
  - the new `codex-acp` evidence strengthens the candidate field, but does not displace the proposal's first-wave choice of Claude Agent ACP plus Gemini CLI ACP
- Runtime evidence used: `None`
- Provenance of key evidence: `/Users/user/Documents/Chainworks Forge/docs/reviews/026-acp-first-runtime-transport-and-goose-decoupling-evidence-pack.md`
- Remaining assumptions:
  - Goose remains the default runtime path through the first additive wave.
  - provider-platform settings remain the machine-local owner for runtime bootstrapping and secrets.
  - Codex ACP remains a promising second-tier candidate rather than a forced first-wave target.
- Remaining blockers: `None`

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top risks:
  1. No live proposal-text blockers remain; the main residual risk is ordinary implementation discipline while extracting Goose-shaped core seams without regressing the preserved default runtime path.
  2. The draft still has no separate proof-owner section, so implementation proof expectations remain implicit rather than centralized.
  3. Naming cleanup across transport, adapter, runtime-profile, and capability seams remains a migration burden even with the owner contract now aligned.
- Top opportunities:
  1. the previous main-text blockers remain closed.
  2. the depended-on additive-profiles note remains aligned with the narrowed `runtime_profile` authority split.
  3. the newly added `codex-acp` evidence broadens the candidate field without destabilizing the first-wave posture or reopening owner ambiguity.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP-shaped canonical runtime vocabulary in core execution code
  - Goose compatibility adapter
  - additive runtime-profile selection
  - first-wave ACP runtime support
  - transport-neutral MCP intent in core runtime code
  - run-snapshot freezing of runtime selection
- Out of scope:
  - destructive Goose removal in the first wave
  - inventing a Forge-owned replacement for ACP
  - guaranteeing perfect parity for every live diagnostic path during the bridge
  - implementation audit or proof execution
- Deferred intentionally:
  - second-wave runtimes beyond Claude Agent ACP and Gemini CLI ACP
  - broader default-runtime replacement decision
  - long-tail operator-grade parity questions for weaker ACP candidates
- Most important baseline refreshes performed:
  - rechecked the main `P026` text against current provider-platform, runtime-truth, and operator-shell owners
  - rechecked the additive runtime-profile companion note because the main proposal explicitly depends on it
  - rechecked the runtime-candidate field after the new `codex-acp` evidence landed
- Most important contradictions with current repo:
  - none remain live in the current reread
- Most important missing or partial states:
  - no proposal-blocking text gaps remain
  - a dedicated proof-owner section remains optional future hygiene, not a readiness blocker

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI findings in the current reread.

### 5.2 UX Findings
- No live UX findings in the current reread.

### 5.3 iOS Architecture Findings
- No live architecture findings in the current reread.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal now depends on a broader ACP candidate field, including a newly added `codex-acp` evidence note.
  Tradeoff: broadening the dependency set could have forced the draft to reopen first-wave target posture or capability-class logic.
  Decision: the refreshed comparison keeps Claude Agent ACP and Gemini CLI ACP as the strongest first-wave pair, while `codex-acp` remains a credible second-tier candidate. The new dependency therefore strengthens the evidence basis without reopening the proposal contract.
  Owner: proposal dependencies, `§5.6`, `§9.3`, `§12.0`, and `/docs/evidence/acp-runtime-candidate-comparison.md`

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Preserve the now-aligned owner split during implementation across catalog/runtime selection, provider-platform bootstrap, run-start freezing, and shell-owned reporting | iOS Architecture | Implementation | Next implementation pass | None | No machine-local runtime/bootstrap truth drifts back into repo YAML | Proposal text aligned |
| P2 | Keep second-wave candidate expansion evidence-driven rather than auto-promoting newly researched ACP runtimes into the first additive wave | iOS Architecture | Proposal / implementation planning | Next planning pass | Candidate-comparison refresh | First-wave scope stays limited to runtimes with proven operator-grade parity | Refreshed comparison field |
| P3 | Add an explicit proof-owner section if future reviewers want tighter handoff hygiene | iOS Architecture | Proposal text | Optional future cleanup | None | Implementation audit can point to a dedicated proving lane | Optional completeness hygiene |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Cross-doc owner alignment | Main proposal and depended-on additive-profiles note preserve the same runtime-profile authority split | `runtime_profiles` examples stay at identity/capability fields | No depended-on proposal note reintroduces machine-local launch authority into repo YAML | Implementation audit | Hold if any depended-on doc reopens the launch-authority ambiguity |
| First-wave runtime posture | Claude Agent ACP and Gemini CLI ACP remain the additive first-wave pair unless a stronger evidence round says otherwise | candidate comparison still ranks them above second-tier options for Forge operator-grade needs | No runtime gets promoted just because it is newly researched; promotion requires live parity evidence | Proposal rereview or follow-up proposal | Hold if future evidence shows the chosen pair are no longer the strongest first-wave targets |
| Main-text stability | The corrected `P026` owner split stays intact | machine-local/bootstrap ownership, runtime truth split, default-Goose preservation, and shell-owned reader continuity remain explicit | No reopen of the previously closed blockers | Implementation audit | Hold if implementation or later edits reintroduce any of those contradictions |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No blocking evidence gaps remain for proposal-readiness. Current repo-local docs, reused baseline artifacts, refreshed candidate evidence, and current code paths are enough to judge the proposal text.

### Open Questions
- `QUESTION-01`: Should the draft add a lightweight proof-owner section now, or leave that to implementation audit discipline later?
