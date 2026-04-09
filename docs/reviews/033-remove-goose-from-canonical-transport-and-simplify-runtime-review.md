# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/033-remove-goose-from-canonical-transport-and-simplify-runtime.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/per-agent-mcp-policy-and-runtime-validation.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/workflow-execution-engine.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R4.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - repo-level current-system map
  - stable transport, MCP, provider-platform, engine, and operator-shell references
- Baseline refreshed:
  - targeted code refresh for runtime dispatch, Goose compatibility, MCP/YAML ownership, frozen run/report truth, operator shell surfaces, and gate ownership
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - none present
- Targeted context refresh performed:
  - yes, repo-local only
- External research used: `None`
- Research pack:
  - none
- Sources reused:
  - stable reference docs and current-system baseline artifacts
- Sources refreshed:
  - current transport/provider/MCP code paths
  - prerequisite `P030` implementation audit state
- Time-sensitive external guidance:
  - none
- Code areas inspected:
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/DSL/AgentCatalog.swift`
  - `Chainworks Forge/DSL/YAMLValidator.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `Chainworks Forge/Views/FirstRunSetupWizard.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift`
  - `scripts/test-gate.sh`
- Current repo contradictions found:
  - `P033` depends on `P030`, but `P030` is still `Not Implemented` / `Not Ready`
  - the current canonical MCP model is still repo-owned through `mcp_profile`, `mcp_profiles`, and `mcp_server_registry`
  - operator shell, onboarding, and runtime-trust copy are still Goose-first
  - core runtime construction still explicitly resolves and carries Goose transport
  - no `proposal-033` proof gate exists yet
- Runtime evidence used: `None`
- Provenance of key evidence:
  - local proposal/docs + stable baseline + targeted code inspection + adjacent implementation-audit artifact
- Remaining assumptions:
  - `P033` is intended as a post-`P030` delta proposal, not overlapping transport migration work
- Remaining blockers:
  - unmet dependency on `P030`
  - unspecified MCP/YAML migration contract
  - unspecified operator-surface migration and proof-gate ownership

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Weak`
- Top risks:
  1. `P033` starts from a prerequisite that is not yet closed: `P030` still lacks full second-wave proof and adapter-aware MCP readiness.
  2. The proposal removes `mcp_profile` / `mcp_server_registry` from the canonical repo model without defining the replacement schema, compiler path, persistence compatibility, or report/comparison migration.
  3. Goose-first operator surfaces are already deeply implemented, but the proposal does not own a surface-by-surface migration plan for onboarding, readiness, runtime provenance, or compatibility fallback.
- Top opportunities:
  1. Recast `P033` as a strict delta over the actually implemented post-`P030` baseline, with an explicit prerequisite evidence gate.
  2. Add one canonical MCP migration contract spanning YAML schema, compiler, run snapshot, preflight, report, and comparison.
  3. Add one operator migration matrix for Settings, First Run Wizard, Pilot Readiness, Start Run, runtime provenance, and Goose compatibility surfaces.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP-first runtime dispatch
  - Goose compatibility-only packaging
  - default runtime migration away from Goose
  - post-Goose MCP ownership simplification
  - proof/doc updates for the simplified runtime
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from all system-level settings
  - weakening execution/recovery/report truth
- Deferred intentionally:
  - none explicitly beyond compatibility retention
- Most important baseline refreshes performed:
  - current transport and Goose-compatibility owner chain
  - current canonical MCP/YAML ownership
  - frozen run/report MCP truth readers
  - current operator shell Goose-first surfaces
  - current proof-gate ownership
- Most important contradictions with current repo:
  - `P030` is still not implementation-ready
  - current DSL and report truth still depend on repo-owned MCP structures that `P033` removes
  - operator onboarding and runtime-trust surfaces still explicitly frame Goose as canonical
- Most important missing or partial states:
  - no replacement schema for backend-profile-owned MCP intent
  - no operator-facing trust/remediation matrix for ACP default vs Goose compatibility
  - no explicit `proposal-033` verification lane

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | High | Complete | 0 | 1 | 0 | 0 |
| UX | Amber | High | Complete | 0 | 1 | 0 | 0 |
| iOS Architecture | Red | High | Complete | 2 | 2 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- Finding ID: `UI-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-07`, `DOC-09`, `NAV-01`, `NAV-02`, `NAV-03`, `NAV-04`, `NAV-05`, `REAL-03`
  Why it matters: Acceptance `7` says operator-facing docs and onboarding must stop implying Goose is canonical, but the current app already encodes Goose-first language across `ProviderSettingsView`, `FirstRunSetupWizard`, `PilotReadinessView`, `IdeaListView`, and `GooseProviderConnectionAssistantView`. The proposal never enumerates those surfaces or defines what remains as compatibility-only UI versus what becomes ACP-first. That leaves too much room for partial or contradictory visual migration.
  Recommended fix: add a surface migration matrix covering Settings, First Run Wizard, Pilot Readiness, Start Run / live runtime guidance, runtime provenance/report surfaces, and Goose Assistant.
  Acceptance criteria:
  - every current Goose-first surface is listed explicitly
  - each surface says whether Goose stays visible there as compatibility-only, disappears, or changes wording
  - “Goose-first” and “Goose-backed live execution” copy is replaced by a defined ACP-default/compatibility vocabulary
  Confidence: `High`

### 5.2 UX Findings
- Finding ID: `UX-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-09`, `NAV-02`, `NAV-03`, `INT-04`, `REAL-03`
  Why it matters: The current operator trust model only distinguishes fixture and Goose runtime provenance. `P033` keeps Goose as a compatibility adapter, but it never defines how operators will understand ACP-default versus Goose-compatibility runs, when compatibility fallback is appropriate, or how degraded/missing-ACP conditions surface in preflight and recovery guidance. That creates a trust gap right in the shell/operator layer.
  Recommended fix: add one runtime-trust and remediation taxonomy for the post-Goose world, covering ACP-default success, Goose compatibility path, compatibility-only fallback, and compatibility unavailable states.
  Acceptance criteria:
  - runtime provenance states are defined for ACP-default and Goose-compatibility paths
  - Start Run, Pilot Readiness, reports, and comparison all use the same trust vocabulary
  - the proposal defines the operator-visible next action when ACP-default is unavailable but Goose compatibility remains possible
  Confidence: `High`

### 5.3 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `DOC-02`, `REAL-01`, `TEST-01`
  Why it matters: `P033` is explicitly framed as work that starts “after second-wave ACP providers are proven,” but the current prerequisite proposal is still `Not Implemented` and `Not Ready`. Starting transport simplification before `P030` closes its proof and adapter-aware MCP gaps would retire the canonical Goose path before the replacement path is actually proven.
  Recommended fix: turn the dependency into a hard evidence gate. `P033` should not be implementation-ready until `P030` reaches Green implementation readiness or its truth is promoted into a stable successor reference.
  Acceptance criteria:
  - `P033` lists a concrete prerequisite readiness condition, not just a dependency link
  - the proposal cannot start while `P030` still lacks all-family proof or adapter-aware MCP readiness
  - rereview of `P033` is gated on current-head evidence, not on intent
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `DOC-06`, `DOC-08`, `MAP-04`, `MAP-05`, `MAP-06`, `MAP-07`, `DATA-01`, `DATA-02`, `REAL-02`
  Why it matters: Section `4.6` removes `mcp_profile` and `mcp_server_registry` from the canonical repo model and says `backend_profile` becomes the only repo-owned execution contract. But current repo truth is the opposite: MCP intent is catalog-owned per agent, validated in YAML, frozen into resolved agents, persisted into run/execution/report data, and surfaced in reports/comparison. The proposal does not define the replacement backend-profile schema, the compiler migration, the frozen-run compatibility contract, or how operator-visible MCP truth survives the rewrite.
  Recommended fix: add a full MCP migration contract that covers the new YAML schema, compile-time ownership, frozen run snapshot compatibility, preflight/readiness validation, report/comparison readers, and back-compat for existing runs/artifacts.
  Acceptance criteria:
  - the replacement backend-profile MCP schema is explicit
  - compiler and validator migration from `agent.mcp_profile` is explicit
  - run snapshot, `AgentExecution`, report, and comparison compatibility rules are explicit
  - machine-local MCP realization authority is defined without losing current requested/predicted/actual/denied truth
  Confidence: `High`

- Finding ID: `ARCH-033-003`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-05`, `MAP-02`, `MAP-03`, `INT-02`, `REAL-04`
  Why it matters: `P033` says Goose becomes compatibility-only and core orchestration should have no Goose-shaped canonical assumptions. But current core still explicitly resolves Goose transport in `ExecutionService`, carries Goose transport for cancellation, and defaults runtime registry access to Goose in `RuntimeSessionBridge`. The proposal never defines one owner matrix for what remains in core, what moves into compatibility packaging, what gets renamed, and what stays as legacy proof tooling.
  Recommended fix: add a module/owner matrix for `ExecutionService`, transport factory, executor/session bridge, compatibility-only transports, fixture lanes, and Goose-specific diagnostics/bootstrap helpers.
  Acceptance criteria:
  - every Goose-owned type or path that remains after the migration is explicitly classified as `core`, `compatibility-only`, or `removed`
  - the proposal names the core abstraction boundaries that survive the simplification
  - the proposal names what happens to cancellation, fixtures, and diagnostics after Goose is no longer canonical
  Confidence: `High`

- Finding ID: `ARCH-033-004`
  Severity: `High`
  Evidence IDs: `DOC-01`, `MAP-09`, `METRIC-01`, `TEST-02`, `TEST-03`, `REAL-05`
  Why it matters: The rollout sequence says to run focused proof gates for the post-Goose transport shape, but the repo currently has no `proposal-033` gate and no defined proof owner for the MCP/YAML migration plus ACP-default / Goose-compatibility behavior. Without an explicit gate, implementation could declare success without reproving the new canonical runtime contract.
  Recommended fix: define the exact verification owner for `P033`, ideally a dedicated `proposal-033` gate with named test suites and proof expectations.
  Acceptance criteria:
  - a concrete same-tree proof lane is named for `P033`
  - the gate proves ACP-default dispatch, Goose-compatibility fallback, and MCP/report truth after the YAML migration
  - proof obligations are aligned with acceptance criteria `1` through `8`
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants Goose to remain available as compatibility-only while current operator shell surfaces are explicitly Goose-first.
  Tradeoff: aggressive retirement reduces conceptual complexity, but incomplete shell migration would destroy operator trust and leave contradictory setup guidance.
  Decision: define one surface-by-surface migration matrix before implementation starts.
  Owner: proposal author

- Conflict: the proposal wants `backend_profile` to become the sole repo-owned MCP intent carrier while the current system freezes MCP intent per agent and exposes it in run/report truth.
  Tradeoff: a flatter model may be cleaner long-term, but an under-specified rewrite would break compiler, persistence, and reporting contracts.
  Decision: add a full migration contract rather than a target-state statement only.
  Owner: proposal author

- Conflict: the proposal treats transport simplification as a next step after second-wave ACP proof, but the prerequisite slice is still not fully closed.
  Tradeoff: starting early could accelerate refactor work, but it would also couple simplification to an unfinished runtime/mcp slice.
  Decision: make prerequisite evidence gating explicit and fail closed until it is met.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add a hard prerequisite evidence gate that blocks `P033` until `P030` is truly implementation-ready | iOS Architecture | Proposal author | Before implementation | current `P030` audit state | `P033` cannot be started while `P030` is red | `ARCH-033-001` |
| P0 | Define the canonical MCP migration contract from `mcp_profile` / `mcp_server_registry` to backend-profile-owned intent | iOS Architecture | Proposal author | Before implementation | current DSL, validator, run/report truth | no unresolved schema/compiler/persistence/report gaps remain | `ARCH-033-002` |
| P0 | Add one operator-surface migration matrix for Goose-first onboarding, readiness, start-run, provenance, and compatibility UI | UI/UX | Proposal author | Before implementation | current provider shell and runtime provenance surfaces | every Goose-first surface has an explicit post-migration state | `UI-033-001`, `UX-033-001` |
| P1 | Add one core-vs-compatibility owner matrix for transport, executor, session bridge, cancellation, fixtures, and diagnostics | iOS Architecture | Proposal author | Before implementation | P0 prerequisite decision | no major Goose-owned runtime path is left ambiguous | `ARCH-033-003` |
| P1 | Define a concrete same-tree proof lane for `P033` | iOS Architecture | Proposal author | Before implementation | P0 MCP contract and owner matrix | `proposal-033` proof ownership is explicit and acceptance-aligned | `ARCH-033-004` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Prerequisite closure | `P030` implementation readiness and proof completeness | all-family second-wave proof and adapter-aware MCP readiness are green | no `P033` work starts early | next rereview of `P033` | hold if `P030` remains red |
| MCP/YAML migration | completeness of replacement schema and persistence/report compatibility | explicit schema, compiler path, snapshot compatibility, and reader migration | no canonical MCP truth is lost during migration | next rereview of `P033` | hold if any of compiler, preflight, report, or comparison remains ambiguous |
| Operator migration | consistency of shell/onboarding/runtime-provenance surfaces | all named Goose-first surfaces have explicit post-migration states | no surface still implies Goose is canonical unintentionally | next rereview of `P033` | hold if surface inventory or trust taxonomy is still partial |
| Proof ownership | operational readiness of the post-Goose proof lane | dedicated gate or equivalent canonical proof slice exists | acceptance criteria cannot be satisfied by ad hoc or partial evidence | next rereview of `P033` | hold if no concrete proof lane exists |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The review has enough local proposal/docs/code/baseline evidence; the blockers are proposal-text and dependency readiness issues.

### Open Questions
- QUESTION-01: what exact backend-profile schema replaces the current per-agent MCP model?
- QUESTION-02: which operator surfaces keep Goose compatibility affordances, and which become ACP-default only?
- QUESTION-03: what is the canonical runtime-trust vocabulary after Goose stops being the canonical runtime path?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
