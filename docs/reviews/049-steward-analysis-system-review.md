# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/049-steward-analysis-system.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/forge-steward.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/yaml-dsl-parser.md`
  - `docs/reference/architecture-decisions.md`
- Baseline refreshed:
  - targeted reread of the stable Steward V1 reference
  - targeted reread of the current Rust daemon startup / northbound configuration owners
  - targeted reread of current control-plane JSON artifact writing conventions
  - targeted code refresh for current Swift dossier fallback / context-link behavior
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `control-plane/crates/daemon/src/main.rs`
  - `control-plane/crates/domain/src/commands.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/runs.rs`
  - `control-plane/crates/mcp-server/src/tools/mod.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift`
  - `docs/reference/forge-steward.md`
- Current repo contradictions found:
  - the old `post_run_hook.enabled` blocker is stale: the current draft now correctly keeps the default at `false`
  - the old config-hash blocker is stale: the current draft now correctly binds `§6b` to canonical parsed-object hashing
  - the old manual-trigger blocker is stale: the current draft now correctly binds `§6c` to the existing namespaced MCP `tools/list` / `tools/call` surface
  - one live input-owner gap remains: the proposal still does not define where the daemon gets the current `StewardConfig` and current `AgentCatalogFile` outside per-run YAML paths
  - one live determinism gap remains: the proposal still promises sorted deterministic JSON while modeling artifact-facing maps as `HashMap` and naming only plain `serde_json`
  - one live evidence-parity gap remains: the proposal still drops the stable context-dossier fallback when no runs are implicated
- Remaining blockers:
  - daemon-global Steward config/current-catalog owner chain is still unspecified
  - deterministic JSON artifact contract is still under-specified

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Most prior red basis is closed, but two high-severity architecture gaps remain and one medium parity gap remains`
- Top residual implementation risks:
  1. `§4e` / `§6b` require a parsed `StewardConfig` and parsed `AgentCatalogFile` at daemon startup and for manual Steward runs, but the current daemon still has no global owner for either input; only per-run workflow/catalog paths exist today.
  2. The proposal promises deterministic JSON artifacts with sorted keys and identical reruns, yet its Rust data model is `HashMap`-heavy and its artifact contract only names plain `serde_json`. That is not sufficient to guarantee stable byte-for-byte output.
  3. Stable Steward V1 still builds dossiers for the first five observation runs when no runs are implicated and persists those links as `context`; the proposal still narrows dossiers to implicated runs only.

## 2. Proposal Scope and Completeness
- In scope:
  - daemon-side Steward V1 parity
  - deterministic metrics, anomaly detection, cohorting, dossiers, and persisted recommendations
  - both optional LLM lanes
  - manual and automatic Steward trigger semantics
- Out of scope:
  - implementation audit or gate execution
  - new Steward UI
  - V2 recommendation patches
  - V3 experiments / decisions
- Most important baseline refreshes performed:
  - stable Steward V1 evidence completeness and dossier fallback
  - current Rust daemon startup / run-start input ownership
  - current control-plane JSON artifact writing conventions
- Most important contradictions with current repo:
  - the proposal now matches the stable default-off post-run trigger
  - the proposal still assumes a daemon-global config/catalog substrate that the current control-plane does not yet own
  - the proposal still over-promises deterministic JSON without a canonical map/serializer contract
  - the proposal still omits the stable no-signal dossier fallback

## 3. Proposal Readiness Verdict
- `Readiness = Red`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is not an Evidence Gap Review. Local proposal, baseline, and current-code evidence are sufficient.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Red | High | Complete | 0 | 2 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding.

### 5.2 UX Findings
- No live UX `proposal-text` finding.

### 5.3 Architecture Findings

#### ARCH-001 - Daemon-global Steward config and current-catalog ownership are still unspecified
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-04`, `MAP-01`, `MAP-02`, `MAP-03`, `MAP-04`, `INT-01`
- Proposal refs:
  - `docs/proposals/049-steward-analysis-system.md:301`
  - `docs/proposals/049-steward-analysis-system.md:303`
  - `docs/proposals/049-steward-analysis-system.md:306`
  - `docs/proposals/049-steward-analysis-system.md:412`
  - `docs/proposals/049-steward-analysis-system.md:418`
- Current repo refs:
  - `control-plane/crates/daemon/src/main.rs:27`
  - `control-plane/crates/daemon/src/main.rs:32`
  - `control-plane/crates/domain/src/commands.rs:16`
  - `control-plane/crates/domain/src/commands.rs:27`
  - `control-plane/crates/graphql-server/src/schema.rs:99`
  - `control-plane/crates/graphql-server/src/schema.rs:123`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:16`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:27`
- Why it matters:
  - The draft now correctly describes how Steward should hash the current config and current catalog, and its service signature expects both a parsed `StewardConfig` and a parsed `AgentCatalogFile`. But the current daemon still has no global owner that can supply either input. `main.rs` only reads `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`; the current northbound start surfaces only carry per-run workflow/catalog YAML paths; and there is still no daemon-owned steward-config path or canonical current-catalog path. As written, implementers would have to invent where startup hash checks, manual `steward.run_analysis`, and optional LLM lane agent resolution get their “current” config objects from.
- Required fix:
  - Add an explicit daemon-owned input contract for both current Steward config and current catalog.
  - Name the owner chain: startup config source, manual-trigger source, and the file(s) that load and pass those parsed objects into Steward.
  - If the intent is to bind Steward to a single repo-local config path (for example `examples/steward/steward_config.yaml` + a canonical catalog path), make that explicit and define how daemon mode resolves those paths independently of per-run YAML paths.

#### ARCH-002 - Deterministic JSON is still under-specified against the proposal’s own `HashMap` model
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-04`, `MAP-05`, `INT-02`, `TEST-01`
- Proposal refs:
  - `docs/proposals/049-steward-analysis-system.md:110`
  - `docs/proposals/049-steward-analysis-system.md:117`
  - `docs/proposals/049-steward-analysis-system.md:123`
  - `docs/proposals/049-steward-analysis-system.md:131`
  - `docs/proposals/049-steward-analysis-system.md:166`
  - `docs/proposals/049-steward-analysis-system.md:454`
  - `docs/proposals/049-steward-analysis-system.md:496`
- Current repo refs:
  - `control-plane/crates/engine/src/executor.rs:824`
- Why it matters:
  - The draft now explicitly requires deterministic artifacts, sorted keys, and identical reruns on identical data. But its proposed Rust structs still use multiple `HashMap<String, ...>` fields for artifact-visible content (`stage_latency_medians`, `retries_per_stage_mean`, `cost_by_stage_family`, `loop_counters`, `cost_by_stage`, `cost_by_agent`) while the artifact contract names only plain `serde_json`. That is not enough to guarantee stable key order or byte-for-byte output across runs. The current control-plane artifact writer still uses `serde_json::to_string_pretty(...)`, which reinforces that no canonical sorted-key JSON writer currently exists in the daemon.
- Required fix:
  - Either replace all artifact-visible `HashMap` fields with deterministic containers (`BTreeMap` or explicitly sorted arrays), or add a named canonical JSON serializer/writer module to the proposal and file inventory.
  - Tie the determinism acceptance criteria to that owner so `AC-7` does not rely on accidental map ordering.

#### ARCH-003 - The stable context-dossier fallback is still missing from the proposal
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-03`, `MAP-06`, `INT-03`
- Proposal refs:
  - `docs/proposals/049-steward-analysis-system.md:45`
  - `docs/proposals/049-steward-analysis-system.md:294`
  - `docs/proposals/049-steward-analysis-system.md:448`
  - `docs/proposals/049-steward-analysis-system.md:499`
- Current repo refs:
  - `docs/reference/forge-steward.md:35`
  - `docs/reference/forge-steward.md:38`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:104`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:109`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:275`
  - `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift:282`
- Why it matters:
  - Stable Steward V1 does not stop at implicated-run dossiers. When no runs are implicated, it still writes dossiers for up to five observation runs and persists those observation-run links as `context`. That keeps evidence completeness intact for inconclusive and no-signal analyses, and it feeds richer inputs into the optional LLM lanes. The proposal’s pipeline, artifact list, and acceptance text still narrow dossiers to implicated runs only, so an implementer can ship a thinner evidence model while still believing they matched V1.
- Required fix:
  - Amend the pipeline, dossier builder contract, artifact section, and acceptance criteria to state: build dossiers for implicated runs, or for the first five observation runs when none are implicated.
  - Explicitly persist those no-signal observation-run links with role `context`.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  - the proposal claims full stable Swift V1 parity, but it still leaves the daemon-global config/catalog owner chain undefined
  - decision needed: choose the canonical daemon source for current `steward_config.yaml` and current `AgentCatalogFile`
- Conflict:
  - the proposal promises deterministic sorted-key JSON while modeling artifact-visible collections as `HashMap`
  - decision needed: deterministic container types vs. canonical serializer module

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Add an explicit daemon-owned source-of-truth contract for current Steward config and current catalog, including startup and manual-trigger resolution | Architecture | proposal author | Before next review | current daemon startup and northbound config surfaces | proposal no longer requires implementers to invent how `StewardConfig` / `AgentCatalogFile` are sourced | `ARCH-001` |
| P1 | Make the deterministic JSON contract executable by naming deterministic containers or a canonical JSON writer | Architecture | proposal author | Before next review | artifact schema section, file inventory, acceptance | `AC-7` no longer depends on accidental `HashMap` ordering | `ARCH-002` |
| P2 | Restore the no-signal context-dossier fallback to the pipeline, artifacts, and acceptance text | Architecture | proposal author | Before next review | stable Swift `StewardAnalysisService` behavior | proposal explicitly preserves context dossiers and `context` run links | `ARCH-003` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Config / catalog owner chain | whether the daemon has an explicit source for current Steward config and current catalog | proposal names startup/manual-trigger input owners and owning files | no silent repo-root / cwd assumption | next proposal review | hold if `§4e` / `§6b` still rely on unspecified config objects |
| Artifact determinism | whether identical data produces identical Steward JSON artifacts | deterministic map/container choice or canonical writer is named | no plain `HashMap` + plain `serde_json` ambiguity | next proposal review | hold if sorted-key determinism is still only aspirational |
| Evidence completeness parity | whether no-signal analyses still emit useful dossiers and `context` links | pipeline and AC explicitly mention the first-five observation-run fallback | no regression to implicated-only dossiers | next proposal review | hold if proposal still narrows dossiers to implicated-only |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal, baseline, and current-code evidence are sufficient.

### Open Questions
- QUESTION-01: Should the daemon own Steward config/current-catalog paths through startup env/config, or through a more explicit persisted workspace/repo binding?
- QUESTION-02: Does the author want deterministic JSON by container choice (`BTreeMap` / sorted vectors) or by a reusable canonical writer utility?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
