# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/015-skill-resolution-and-runtime-injection.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/yaml-dsl-parser.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - operator-shell ownership
  - immutable run-start ownership
  - provider / readiness ownership
- Baseline refreshed:
  - current external skill-bundle contract under `/Users/user/.codex/skills/*`
  - current shared-skill specialist contract for `proposal_review_triad`
  - current shell-owned report / comparison / artifact routes
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/AgentCatalog.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/YAMLValidator.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlan.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Run.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/AgentExecution.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/AgentCatalogView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/PilotReadinessView.swift`
  - `/Users/user/.codex/skills/proposal-review-triad/SKILL.md`
  - `/Users/user/.codex/skills/proposal-review-triad/references/product-review-rubric.md`
  - `/Users/user/.codex/skills/proposal-review-triad/references/ux-financial-rubric.md`
  - `/Users/user/.codex/skills/proposal-implementation-audit/SKILL.md`
  - `/Users/user/Documents/Chainworks Forge/examples/agents/agents.yaml`
- Targeted context refresh performed: `Yes`
- External research used: `None`
- Research pack: `None`
- Sources reused: current repo baselines only
- Sources refreshed: current external skill bundle shape, specialist-mode contract, current report/comparison/artifact owners
- Time-sensitive external guidance: `None`
- Code areas inspected:
  - YAML skill parsing and referential validation
  - compile-time `ResolvedAgent` assembly
  - immutable `Run` + `RunStartSnapshot` ownership
  - per-execution provenance persistence
  - prompt assembly path via `GooseSessionBridge`
  - current report / comparison / artifact / readiness owners
- Current repo contradictions found:
  - none live after the current proposal edits
  - previous blockers around `SKILL.md`, role-to-mode mapping, frozen owner, shell-owned visibility, and raw-vs-injected provenance are now explicitly closed in text
- Runtime evidence used: `None`
- Repeat freshness note: `No-delta repeat on 2026-04-03; proposal hash unchanged from the prior green pass.`
- Provenance of key evidence: `/Users/user/Documents/Chainworks Forge/docs/reviews/015-skill-resolution-and-runtime-injection-evidence-pack.md`
- Remaining assumptions:
  - MVP intentionally imports executable skill truth from `SKILL.md` plus declared specialization, not the full Codex skill runtime
  - companion bundle files remain provenance-visible unless a later proposal promotes them into executable truth
- Remaining blockers: `None`

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top risks:
  1. No live proposal-text blockers remain; the main remaining risk is ordinary implementation discipline when wiring skill resolution into compilation, injection, provenance, and shell-owned visibility.
  2. Future lazy or partial injection would need a new proposal-owned raw-vs-injected truth contract; the current draft is correctly fail-closed for MVP.
  3. Skill-specific specialization registries will need careful upkeep so shared skills keep mapping to real current mode contracts.
- Top opportunities:
  1. Implementation can now target the repo’s actual external skill bundle shape instead of inventing a parallel loader contract.
  2. Shared-skill reviewers can now preserve current specialist behavior through explicit mode mapping rather than weak generic role text.
  3. Provenance and operator visibility are now anchored to current immutable-run and shell-owned owners, which reduces downstream drift.

## 2. Proposal Scope and Completeness
- In scope:
  - external / inline / builtin skill resolution
  - runtime prompt injection
  - specialist role / mode behavior
  - frozen-run and per-execution provenance
  - preflight readiness reporting
  - shell-owned operator visibility
- Out of scope:
  - implementation audit
  - build / run / simulator proof
  - full Codex skill-runtime emulation
  - external research
- Deferred intentionally:
  - lazy or partial skill injection
  - broader rollout / flag strategy
  - future promotion of bundle companions into executable truth
- Most important baseline refreshes performed:
  - verified current external skills are rooted at `SKILL.md`
  - verified `proposal_review_triad` specialization is mode-based
  - verified current run inspection remains shell-owned through report / comparison / artifact routes
- Most important contradictions with current repo:
  - none remain live in the current proposal text
- Most important missing or partial states:
  - no proposal-blocking gaps remain
  - future lazy injection and richer companion execution are intentionally deferred

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
- Conflict: external skill discovery previously opened a second bundle contract.
  Tradeoff: generic markdown-directory loading was simpler on paper, but it diverged from current repo reality.
  Decision: the draft now standardizes on the actual Codex `SKILL.md` bundle contract.
  Owner: Sections 3, 4.1, Appendix A.

- Conflict: shared reviewer roles previously relied on weak generic role text.
  Tradeoff: generic role injection was simpler, but it could not reproduce the current shared-skill specialist contract.
  Decision: the draft now requires explicit skill-specific mode mapping for the current `proposal_review_triad` case.
  Owner: Sections 5.3, 10, Appendix A/B.

- Conflict: previous provenance language allowed execution truth to diverge from stored hash truth.
  Tradeoff: truncation would reduce prompt size, but it would also break auditability.
  Decision: MVP is now fail-closed, with separate raw and injected hashes reserved for truthful future extensions.
  Owner: Sections 6.1-6.4.

- Conflict: previous execution visibility invented a separate inspection lane.
  Tradeoff: a dedicated surface might feel explicit, but it would fork current operator ownership.
  Decision: the draft now extends shell-owned report / comparison / artifact routes only.
  Owner: Sections 3, 8, 9, 10.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Implement `ExternalSkillLoader` against the declared `SKILL.md` bundle contract and prove it on current `.codex/skills/*` fixtures | iOS Architecture | Implementation | Next implementation pass | None | Current local external skills resolve without fallback ambiguity | Proposal text aligned |
| P1 | Implement skill-specific specialization registry support for `proposal_review_triad` and prove real mode-mapped prompt divergence | UX / iOS Architecture | Implementation | Next implementation pass | Loader support | `product_owner`, `ux_designer`, `ui_designer`, and `architect` produce genuinely different injected content | Proposal text aligned |
| P1 | Extend immutable `Run` + `RunStartSnapshot` plus `AgentExecution` provenance exactly as specified | iOS Architecture | Implementation | Next implementation pass | Compiler + orchestrator changes | Raw and injected skill truth remain auditable and consistent | Proposal text aligned |
| P2 | Extend shell-owned report / comparison / artifact surfaces and `PilotReadinessView` with skill truth visibility | UI | Implementation | Next implementation pass | Provenance wiring | Operators can inspect skill truth without a parallel surface | Proposal text aligned |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| External skill resolution | Current local Codex bundles resolve under the declared loader contract | Fixture coverage for `SKILL.md` bundles plus explicit missing-file failures | No ambiguous fallback to arbitrary markdown files | Implementation audit | Hold if current `.codex/skills/*` bundles still fail the declared contract |
| Shared-skill specialization | Shared reviewer roles produce distinct injected instructions via real mode mapping | Integration coverage for `proposal_review_triad` role set | No silent fallback to the generic default contract for required specialist roles | Implementation audit | Hold if `A4` can pass without real mode-specific divergence |
| Frozen truth and provenance | Raw resolved skill truth and injected execution truth remain consistent and inspectable | Snapshot round-trip plus provenance tests | No successful execution hashes one artifact while executing another | Implementation audit | Hold if any success path requires truncation or other silent mutation of executable content |
| Operator visibility | Skill truth remains inside current shell-owned routes | UI smoke coverage on report / comparison / artifact / readiness owners | No new parallel inspection surface | Implementation audit | Hold if execution-time skill visibility depends on a new standalone owner |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No blocking evidence gaps remain for proposal-readiness. Runtime proof is intentionally deferred to the later implementation audit.

### Open Questions
- `QUESTION-01`: If future proposals promote companion bundle files into executable truth, should that happen through explicit flattening at compile time or through typed companion references carried into runtime?
