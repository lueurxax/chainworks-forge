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
  - targeted code refresh for current MCP contract richness
  - targeted code refresh for Goose owner inventory and operator surfaces
  - targeted code refresh for runtime-trust readers and persisted legacy values
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - none present
- Targeted context refresh performed:
  - yes, repo-local only
- External research used: `None`
- Sources refreshed:
  - current transport/provider/MCP code paths
  - current trust readers and fixture/preview references
  - prerequisite `P030` implementation audit state
- Runtime evidence used: `None`
- Current repo tensions found:
  - `P033` now correctly hard-gates on `P030`, but `P030` is still red today
  - the current canonical MCP contract is richer than the proposal's current `mcp_intent` description
  - the repo still has more Goose-owned files and surfaces than the proposal explicitly inventories
  - historical shell/report/previews still use legacy `server_unverified` / `server_verified` trust values

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Improved but still blocked`
- Top improvements since the prior pass:
  1. `P033` now fail-closes the `P030` dependency instead of treating it as narrative only.
  2. The proposal no longer deletes MCP structures in one step; it now defines a three-phase migration.
  3. The proposal now introduces an explicit operator-surface migration table and a post-Goose trust vocabulary.
- Remaining blockers:
  1. The new `mcp_intent` contract is still too thin compared with the current required/optional/fallback/runtime-mapping MCP model and frozen report truth.
  2. The “every Goose-touching file/surface” inventory is still incomplete; important current owners are missing from the proposal.
  3. The trust-vocabulary rewrite does not yet define how historical `server_unverified` / `server_verified` runs keep rendering correctly.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP-first runtime dispatch
  - Goose compatibility-only packaging
  - default runtime migration away from Goose
  - phased MCP ownership migration
  - operator-surface migration and trust vocabulary
  - proof/doc updates for the simplified runtime
- Out of scope:
  - removing Goose support entirely
  - deleting Goose tooling from all system-level settings
  - weakening execution/recovery/report truth
- Most important baseline refreshes performed:
  - richer current MCP schema and runtime semantics
  - current Goose file/surface inventory
  - current persisted trust values and shell/report readers
- Most important remaining proposal gaps:
  - exact `backend_profile.mcp_intent` schema and semantics
  - exhaustive core/compatibility inventory for Goose files and surfaces
  - explicit migration/read fallback for historical trust values

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | High | Complete | 0 | 1 | 0 | 0 |
| UX | Amber | High | Complete | 0 | 1 | 0 | 0 |
| iOS Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- Finding ID: `UI-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `NAV-01`, `NAV-02`, `NAV-03`, `NAV-04`, `NAV-05`, `NAV-06`, `REAL-02`
  Why it matters: The proposal now has an operator-surface migration table, but it still does not cover the full current Goose-first shell. `PilotReadinessView`, `GooseProviderConnectionAssistantView`, and the runtime-provenance badge/history surfaces are live product surfaces today, and the proposal’s acceptance criteria say every Goose-first surface should be migrated. As written, implementation could finish with a clean settings/setup story but still leave Goose-first affordances in readiness, assistant, and shell history surfaces.
  Recommended fix: expand the surface matrix into one exhaustive operator inventory that includes setup, readiness, assistant, live start-run copy, runtime provenance/history badges, and any compatibility-only destinations that remain operator-visible.
  Acceptance criteria:
  - every currently Goose-first operator surface is named explicitly
  - each named surface says whether it becomes ACP-default, compatibility-only, or removed
  - shell/history surfaces are included, not only setup flows
  Confidence: `High`

### 5.2 UX Findings
- Finding ID: `UX-033-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-09`, `MAP-05`, `NAV-06`, `INT-04`, `REAL-03`
  Why it matters: The new trust vocabulary is directionally correct, but the proposal does not define how historical runs with `server_unverified` / `server_verified` continue to render in the shell, reports, comparison, previews, and recovery surfaces. Without an explicit forward-mapping or reader fallback, the operator can lose trust continuity the moment the vocabulary changes.
  Recommended fix: add a trust-migration contract covering stored `Run.runtimeTrustLevel` values, reader fallback rules, preview fixtures, and compatibility labels for historical runs.
  Acceptance criteria:
  - legacy `server_unverified` / `server_verified` values have an explicit forward mapping or reader fallback
  - `RunsHomeView`, report/comparison surfaces, previews, and recovery all share the same mapping
  - the proposal states whether old values are migrated in storage, mapped on read, or both
  Confidence: `High`

### 5.3 iOS Architecture Findings
- Finding ID: `ARCH-033-001`
  Severity: `Critical`
  Evidence IDs: `DOC-01`, `DOC-06`, `MAP-04`, `MAP-05`, `MAP-06`, `DATA-01`, `DATA-02`, `REAL-01`
  Why it matters: `P033` correctly switched from one-step deletion to a phased MCP migration, but the replacement contract is still too thin to implement safely. The proposal currently says `backend_profile` gets an optional `mcp_intent` field declaring required MCP servers, while current repo truth still depends on `required_extensions`, `optional_extensions`, `fallback_policy`, runtime namespace mappings, and frozen requested/predicted/actual/denied MCP truth. Without an explicit replacement schema and freeze/report mapping, Phase 1 cannot be implemented without guessing which current semantics survive.
  Recommended fix: define the exact `mcp_intent` schema and its compatibility rules before implementation. The proposal needs to specify how required vs optional MCP intent, fallback behavior, runtime mappings, and frozen report truth migrate through Phases 1-3.
  Acceptance criteria:
  - `backend_profile.mcp_intent` has an explicit schema, not only a name
  - the schema covers required vs optional MCP intent and fallback behavior, or explicitly defines what replaces them
  - compile-time freezing, persisted run/execution truth, report/comparison readers, and machine-local realization all have explicit migration rules
  - Phase 1 and Phase 3 both preserve operator-visible MCP truth semantics without guesswork
  Confidence: `High`

- Finding ID: `ARCH-033-002`
  Severity: `High`
  Evidence IDs: `DOC-01`, `MAP-02`, `MAP-03`, `MAP-07`, `MAP-08`, `REAL-02`
  Why it matters: Section `5.3` says every Goose-touching file will be classified as core or compatibility, but the current proposal inventory is still incomplete. The repo still has `FixtureGooseTransport.swift`, `GooseProviderConnectionAssistant.swift`, `GooseProviderConnectionAssistantView.swift`, and other Goose-named or Goose-owned surfaces outside the current table. That leaves room for a partial refactor where “canonical Goose” is removed from the main transport path but retained accidentally in fixtures, assistants, or proof tooling.
  Recommended fix: expand the core/compatibility owner matrix to include every current Goose-owned file and explicitly state what happens to fixtures, assistant services, preview/test support, and legacy proof helpers.
  Acceptance criteria:
  - the file inventory includes all current Goose-owned files, not just transport classes
  - fixture/proof/test support has an explicit classification
  - provider-assistant service/view ownership is explicit
  - no Goose-named owner remains unclassified after rereview
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict: the proposal wants a simpler ACP-first MCP model while current repo truth exposes a richer per-agent MCP contract to runtime, reports, and comparison.
  Tradeoff: simplification is valuable, but a thinner replacement contract would silently remove current semantics.
  Decision: specify the replacement MCP schema and frozen-truth compatibility before implementation starts.
  Owner: proposal author

- Conflict: the proposal wants Goose to remain as compatibility-only while the current shell and helper layers still expose Goose-specific journeys, fixtures, and history labels.
  Tradeoff: a smaller matrix is easier to read, but an incomplete inventory will leave canonical-Goose leftovers in operator experience and proof tooling.
  Decision: expand the owner/surface inventory to every current Goose-owned file and operator-visible surface.
  Owner: proposal author

- Conflict: the proposal introduces better trust vocabulary, but current persisted runs and shell readers still use legacy `server_*` values.
  Tradeoff: renaming the vocabulary improves future clarity, but without a migration/read policy it breaks historical continuity.
  Decision: add a reader-compatible trust migration contract.
  Owner: proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Define the exact `backend_profile.mcp_intent` schema and all freeze/report/realization compatibility rules | iOS Architecture | Proposal author | Before implementation | current MCP contract and report truth | no MCP migration step requires inference from implementers | `ARCH-033-001` |
| P0 | Expand the Goose owner inventory to every current file/surface, including fixtures and assistant flows | UI/Architecture | Proposal author | Before implementation | current Goose file/surface set | every current Goose-owned file and operator-visible surface is classified | `UI-033-001`, `ARCH-033-002` |
| P0 | Add legacy trust-value migration/read rules for historical runs and shell/report/previews | UX/Architecture | Proposal author | Before implementation | current `runtimeTrustLevel` readers | historical runs remain legible under the new trust vocabulary | `UX-033-001` |
| P1 | Name the exact `proposal-033` suites/evidence outputs expected in the focused gate | iOS Architecture | Proposal author | Before implementation | `P030` prerequisite gate | proof ownership is operational, not only conceptual | follow-up to `TEST-02`, `TEST-03` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| MCP migration | completeness of replacement MCP schema and truth preservation | explicit schema, fallback semantics, run/report compatibility, runtime mapping | no current MCP truth is dropped silently | next rereview of `P033` | hold if any phase still requires inference |
| Goose owner migration | completeness of core/compatibility inventory | all current Goose-owned files and surfaces are classified | no Goose-owned path remains “implicit” | next rereview of `P033` | hold if fixtures/assistants/history surfaces remain unnamed |
| Trust migration | historical trust continuity | explicit mapping for legacy stored values and readers | no historical run/report surface breaks or becomes ambiguous | next rereview of `P033` | hold if shell/report/previews still rely on undefined legacy mapping |
| Dependency closure | `P030` readiness | all-family second-wave proof and adapter-aware MCP readiness are green | no `P033` implementation starts early | next rereview of `P033` | hold if `P030` remains red |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. The review has enough local proposal/docs/code/baseline evidence; the blockers are proposal-text and dependency-readiness issues.

### Open Questions
- QUESTION-01: what exact schema replaces the current richer per-agent MCP contract?
- QUESTION-02: which Goose compatibility surfaces remain operator-visible after migration?
- QUESTION-03: how are historical trust values preserved across shell, report, comparison, and preview readers?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
