# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/provider-platform.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/acp-runtime-transport.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/per-agent-mcp-policy-and-runtime-validation.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/030-acp-second-wave-runtime-profiles-codex-auggie-junie_IMPLEMENTATION_AUDIT_R2.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - repo-level current-system map
  - stable provider-platform, ACP transport, and MCP validation references
- Baseline refreshed:
  - targeted code refresh for provider families, settings/selection, runtime transport, MCP registry ownership, canonical catalog entries, and `proposal-030` proof gate
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - none present
- Targeted context refresh performed:
  - yes, repo-local only
- External research used: `None`
- Research pack:
  - none
- Sources reused:
  - stable reference docs and existing baseline artifacts
- Sources refreshed:
  - current provider/runtime/MCP code paths and existing `P030` implementation audit
- Time-sensitive external guidance:
  - none
- Code areas inspected:
  - `Chainworks Forge/Providers/ConfiguredProvider.swift`
  - `Chainworks Forge/Providers/ProviderSettingsStore.swift`
  - `Chainworks Forge/Providers/ProviderRegistry.swift`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift`
  - `Chainworks Forge/Engine/RuntimeTransport.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/ACPAdapters/CodexACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/AuggieCLIACPTransport.swift`
  - `Chainworks Forge/Engine/ACPAdapters/JunieCLIACPTransport.swift`
  - `Chainworks Forge/Views/ProviderSettingsView.swift`
  - `Chainworks Forge/Views/PilotReadinessView.swift`
  - `Chainworks Forge/Views/ProviderTroubleshootingPanel.swift`
  - `examples/agents/agents.yaml`
  - `Chainworks ForgeTests/Proposal029Tests.swift`
  - `scripts/test-gate.sh`
- Current repo contradictions found:
  - second-wave provider families, capability enforcement, disabled-provider rollout state, and canonical runtime profiles already exist in repo
  - second-wave transports are still execution stubs
  - live runtime-registry loading is still Goose-centric
  - canonical rich MCP mappings are preserved for `codex`, but `auggie` and `junie` mappings remain unspecified
- Runtime evidence used: `None`
- Provenance of key evidence:
  - local proposal/docs + stable baseline + current code inspection + adjacent implementation-audit artifact
- Remaining assumptions:
  - `P030` is reviewed as a delta proposal over current stable refs
- Remaining blockers:
  - proposal scope/phase ownership is internally inconsistent
  - second-wave registry/MCP contract is still underspecified for Auggie and Junie
  - proof contract is weaker than the full in-scope transport surface

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Mixed`
- Top risks:
  1. `P030` says all three live second-wave transports are in scope, but the rollout-order section still labels only the structural slice as “this proposal”.
  2. MCP registry work is aimed at the right seam, but the proposal still does not lock the concrete runtime-registry authority or explicit zero-MCP policy for `auggie` and `junie`.
  3. Verification remains uneven: acceptance covers all three transports, while the proof section only requires one successful Codex path and looser expectations for the other two.
- Top opportunities:
  1. Convert `P030` into one unambiguous delta plan over current HEAD instead of a mixed “already landed + maybe later phases” document.
  2. Lock one adapter-family matrix for runtime registry source, MCP lane availability, and preflight behavior.
  3. Strengthen the proof gate so implementation cannot claim completion with only partial second-wave execution evidence.

## 2. Proposal Scope and Completeness
- In scope:
  - second-wave ACP runtime onboarding for Codex, Auggie, and Junie
  - provider-platform expansion and rollout gating
  - fail-closed transport selection
  - MCP registry ownership and runtime validation
  - capability enforcement through `ProviderCapabilities`
  - focused `proposal-030` proof gate
- Out of scope:
  - Goose removal
  - hard cutover away from Goose
  - operator-grade claims for second-wave providers
  - generic cross-provider MCP parity
- Deferred intentionally:
  - transport simplification in `P031`
- Most important baseline refreshes performed:
  - provider-platform owner chain
  - first-wave ACP transport baseline
  - current MCP validation ownership
  - current canonical catalog/runtime-profile state
  - current focused proof gate
- Most important contradictions with current repo:
  - `P030` is correctly written as a delta over current HEAD, but it still mixes already-landed structural work with future live-transport phases ambiguously
  - current repo preserves Codex MCP mappings, while Auggie/Junie MCP lane behavior remains undefined
- Most important missing or partial states:
  - one shared operator-facing taxonomy for `disabled`, `misconfigured`, `missing registry`, and `healthy`
  - one explicit runtime-registry and MCP-lane matrix for all second-wave families
  - proof requirements that match the full in-scope transport set

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | Medium | Complete | 0 | 0 | 1 | 0 |
| UX | Amber | Medium | Complete | 0 | 0 | 1 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 2 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- Finding ID: `UI-029-001`
  Severity: `Medium`
  Evidence IDs: `DOC-04`, `NAV-01`, `NAV-02`, `NAV-03`, `INT-01`
  Why it matters: `P030` changes a user-facing provider platform that already has `ProviderSettingsView`, `PilotReadinessView`, troubleshooting, and run-start preflight. The proposal explicitly names a Settings toggle and a preflight/report distinction for disabled providers, but it still does not lock one shared status/copy contract across the full provider shell for `disabled`, `misconfigured`, `missing runtime registry`, and `healthy`. That leaves room for implementation drift and mixed operator messaging.
  Recommended fix: add one cross-surface provider-status contract covering Settings, Pilot Readiness, troubleshooting, and preflight for second-wave providers.
  Acceptance criteria:
  - `disabled`, `misconfigured`, `missing registry`, and `healthy` are defined as distinct operator-visible states
  - the proposal names which surfaces render each state and what next action they show
  - second-wave providers do not inherit Goose-first language when the issue is rollout or registry state
  Confidence: `Medium`

### 5.2 UX Findings
- Finding ID: `UX-029-001`
  Severity: `Medium`
  Evidence IDs: `DOC-04`, `DOC-06`, `NAV-02`, `NAV-03`, `H`
  Why it matters: the proposal correctly separates `provider not enabled` from `capability mismatch` at preflight, but the broader remediation journey is still underspecified. A second-wave provider can currently be intentionally disabled, missing MCP registry support, blocked by missing lanes, or still stubbed at execution time. Without one operator-facing remediation sequence, implementation can scatter conflicting next steps across Settings, Pilot Readiness, troubleshooting, and run start.
  Recommended fix: add a short remediation matrix that maps each blocked second-wave state to one owner surface and one canonical next action.
  Acceptance criteria:
  - every blocked second-wave state has a single primary remediation surface
  - run-start preflight text aligns with provider troubleshooting and pilot-readiness messaging
  - stub-transport failure is explicitly distinguished from rollout-disabled and registry-unavailable states
  Confidence: `Medium`

### 5.3 iOS Architecture Findings
- Finding ID: `ARCH-029-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `REAL-02`, `REAL-04`, `M`
  Why it matters: sections `3`, `3.2`, and acceptance `11` say that `P030` includes end-to-end execution for `codex_acp`, `auggie_cli_acp`, and `junie_cli_acp`. But section `4.7` still labels only “Phase 1 — Structural prerequisites” as “this proposal”, with Codex and Auggie/Junie execution deferred into later phases. That is an internal scope contradiction. It reintroduces exactly the kind of unsafe staged rollout confusion that earlier reviews flagged.
  Recommended fix: make the ownership model explicit. Either keep all three executable transports inside `P030` and rewrite `4.7` accordingly, or split later transport phases into new proposals and remove the all-three execution commitments from `P030`.
  Acceptance criteria:
  - rollout-order text and acceptance criteria describe the same in-scope surface
  - there is no interpretation where structural scaffolding lands while live transport work is “later” but still counted as `P030` complete
  - Codex/Auggie/Junie execution commitments are either all in or explicitly out with new proposal ownership
  Confidence: `High`

- Finding ID: `ARCH-029-002`
  Severity: `High`
  Evidence IDs: `DOC-05`, `DOC-06`, `MAP-05`, `MAP-07`, `INT-03`, `INT-04`, `REAL-03`, `REAL-06`
  Why it matters: `P030` correctly targets the remaining Goose-owned registry seam, but it still does not fully lock the second-wave MCP contract. The proposal says each second-wave provider gets a runtime namespace and new registry conformers, yet only Codex runtime mappings are explicitly preserved in the canonical catalog. It never decides whether `auggie` and `junie` ship with real MCP lanes, or intentionally remain zero-MCP-only and must fail MCP-dependent preflight by design. Without that decision, implementation can invent incompatible registry readers and lane behavior.
  Recommended fix: add one adapter-family matrix covering runtime namespace, registry source, install/readiness owner, and whether MCP lanes are supported, absent by design, or future work for each of `codex`, `auggie`, and `junie`.
  Acceptance criteria:
  - each second-wave family has an explicit MCP policy stance: mapped lanes or zero-MCP-only
  - the proposal names the concrete runtime-registry source or explicitly says none exists for that family
  - preflight behavior for MCP-dependent agents is deterministic per family
  Confidence: `High`

- Finding ID: `ARCH-029-003`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-06`, `MAP-08`, `TEST-01`, `TEST-03`, `REAL-05`
  Why it matters: the verification contract is weaker than the proposal scope. Acceptance `11` says runs routed to all three second-wave transports must stop failing with stub errors, but `3.2` only requires one successful Codex proof path plus “explicit preflight/proof expectations” for Auggie/Junie. That leaves the proposal open to partial execution proof while still claiming closure on a three-transport slice.
  Recommended fix: align proof requirements with the actual scoped surface. Either require per-provider execution proof for all in-scope transports, or narrow the proposal to Codex-first execution with Auggie/Junie explicitly deferred.
  Acceptance criteria:
  - proof gate text makes clear which providers require real execution proof before proposal closure
  - same-tree verification cannot pass with only Codex live proof if Auggie and Junie remain in scope as executable transports
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants to be both an implementation-aligned delta over current HEAD and a phased rollout plan with later execution stages.
  Tradeoff: a phased narrative helps sequencing, but it becomes dangerous when acceptance criteria already claim all-three execution completion.
  Decision: pick one ownership model and reflect it consistently across scope, rollout order, and acceptance.
  Owner: proposal author

- Conflict: stable references still describe a Goose-first registry world, while `P030` is meant to finish the transport-neutral second-wave MCP path.
  Tradeoff: keeping the proposal high-level leaves flexibility, but it also leaves too much room for inconsistent per-runtime behavior.
  Decision: add one explicit adapter-family MCP matrix before implementation starts.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Resolve the internal scope/phase contradiction and decide whether all three executable transports are truly in `P030` | iOS Architecture | Proposal author | Before implementation | current proposal draft | scope, rollout order, and acceptance criteria all describe the same surface | `ARCH-029-001` |
| P0 | Add an adapter-family MCP and runtime-registry authority matrix for Codex, Auggie, and Junie | iOS Architecture | Proposal author | Before implementation | current provider/MCP baseline | no ambiguity remains about lanes, registry source, or preflight behavior per family | `ARCH-029-002` |
| P1 | Strengthen the proof contract so it matches the actual in-scope transport set | iOS Architecture | Proposal author | Before implementation | P0 scope decision | same-tree proof requirements cannot be satisfied by partial execution evidence | `ARCH-029-003` |
| P1 | Lock one operator-facing state/remediation contract for disabled vs broken vs registry-missing second-wave providers | UI/UX | Proposal author | Before implementation | P0/P0 | Settings, Pilot Readiness, troubleshooting, and preflight use one consistent state model | `UI-029-001`, `UX-029-001` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Proposal scope integrity | alignment between scope, rollout order, and acceptance criteria | no contradictory in-scope vs later-phase wording | no structural-only interpretation remains when live transports are claimed in scope | next proposal rereview | hold if `P030` still allows multiple ownership readings |
| Second-wave MCP contract | per-family clarity for lanes, registry source, and blocked behavior | explicit matrix for Codex/Auggie/Junie | no ad hoc runtime-specific registry behavior is needed during implementation | next proposal rereview | hold if Auggie/Junie MCP behavior is still implicit |
| Proof strategy | proof obligations for each in-scope transport | same-tree gate and per-provider proof rows are named | no provider remains in-scope without an explicit proof threshold | next proposal rereview | hold if closure can still be claimed with only partial live execution proof |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking local evidence gap remains. The readiness blockers are proposal-text contradictions and omissions, not missing repo evidence.

### Open Questions
- QUESTION-01: Should `P030` remain the owner for all three live second-wave transports, or should only the structural/Codex slice remain here with Auggie/Junie split later?
- QUESTION-02: Are `auggie` and `junie` intended to support any MCP lanes in this proposal, or are they intentionally zero-MCP-only until a later slice?
- QUESTION-03: If second-wave runtimes require registry validation, what is the canonical registry source and readiness owner per adapter family?
