# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/049-steward-analysis-system.md` | 2026-04-15 | High | The current draft closes the older `post_run_hook`, config-hash, manual-trigger, auditor-dependency, and strategy-validation blockers, but it still leaves the daemon-global config/catalog owner chain unspecified, still over-promises deterministic JSON with `HashMap`-heavy structs, and still omits the stable no-signal dossier fallback. | Review could miss the remaining proposal-first seams after the stale blockers were closed. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should still anchor to the latest reusable baseline before judging daemon Steward parity. | Review could drift from current host-system contracts. | Intake baseline. |
| DOC-03 | `docs/reference/forge-steward.md` | 2026-04-15 | High | Stable Steward V1 still builds dossiers for implicated runs or the first five observation runs when none are implicated, and persists run links with roles `implicated`, `baseline`, or `context`. | Proposal can still under-port evidence completeness while claiming full V1 parity. | Main stable-reference anchor. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current daemon startup/config model is explicit and small: no Steward-specific config/catalog owner exists yet, while MCP tools remain namespaced. | Helps judge whether the proposal names a real daemon owner chain. | Control-plane boundary anchor. |
| DOC-05 | `docs/reference/yaml-dsl-parser.md` and `docs/reference/architecture-decisions.md` | 2026-04-15 | High | Stable provenance hashing is defined as canonical parsed-object hashing rather than raw file-content hashing. | Prevents replaying stale config-hash blockers. | Provenance anchor. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake baseline | 2026-04-15 | High | Fresh enough as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/forge-steward.md` | Partially refreshed | stable Steward V1 evidence completeness and trigger behavior | 2026-04-15 | High | Refreshed narrowly against current Swift Steward code. | Main parity baseline. |
| BASE-03 | `docs/reference/rust-control-plane.md` | Partially refreshed | current daemon startup / northbound ownership conventions | 2026-04-15 | High | Refreshed narrowly against current `daemon`, GraphQL, and MCP code. | Control-plane boundary baseline. |
| BASE-04 | `docs/reference/yaml-dsl-parser.md` and `docs/reference/architecture-decisions.md` | Partially refreshed | canonical config-hashing semantics | 2026-04-15 | High | Refreshed narrowly against proposal `§6b`. | Provenance baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
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
- Assumptions:
  - review mode is `proposal-readiness`
  - stable Steward V1 remains the parity authority unless another proposal explicitly supersedes it
  - the user wants a fresh reread against the current proposal text, not a replay of the older red basis
- Blockers:
  - daemon-global Steward config/current-catalog ownership is still unspecified
  - deterministic JSON artifact ownership is still unspecified

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | no dedicated Steward UI in current shell | Baseline | 2026-04-15 | High | Stable Steward V1 remains an offline observer slice. | Prevents inventing UI blockers. | Keeps review architecture-focused. |
| NAV-02 | daemon startup configuration surface | Targeted refresh | 2026-04-15 | High | Current daemon startup still only owns `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`. | Proposal can still assume config inputs that have no current owner. | Main live seam. |
| NAV-03 | manual MCP trigger surface | Baseline + current repo | 2026-04-15 | High | Current control-plane tool dispatch is still namespaced through `tools/list` / `tools/call`. | Prevents replaying the old manual-trigger blocker. | Closure note. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/daemon/src/main.rs` | Rust daemon | startup config owner | 2026-04-15 | High | Daemon startup currently reads only `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`; there is no Steward config path or current catalog path. | Proposal can still assume startup inputs that the daemon cannot actually supply. | Live blocker proof. |
| MAP-02 | `control-plane/crates/domain/src/commands.rs` | Rust domain | northbound start-run command contract | 2026-04-15 | High | `StartRunCmd` carries per-run workflow/catalog YAML paths only; it has no Steward config field and no daemon-global catalog owner. | Confirms current config/catalog ownership is per-run, not daemon-global. | Live blocker proof. |
| MAP-03 | `control-plane/crates/graphql-server/src/schema.rs` | Rust northbound | GraphQL start-run mutation | 2026-04-15 | High | GraphQL `start_run` still forwards only workflow/catalog YAML paths through `StartRunCmd`; there is no Steward config/current-catalog input. | Confirms the missing owner gap is real across northbound surfaces. | Live blocker proof. |
| MAP-04 | `control-plane/crates/mcp-server/src/tools/runs.rs` | Rust northbound | MCP `runs.start` tool | 2026-04-15 | High | MCP `runs.start` mirrors the same input contract: workflow/catalog per run, no Steward config/current-catalog owner. | Confirms the missing owner gap is not GraphQL-only. | Live blocker proof. |
| MAP-05 | `control-plane/crates/engine/src/executor.rs` | Rust engine | current JSON artifact writing convention | 2026-04-15 | High | Current artifact writing still uses plain `serde_json::to_string_pretty(...)`; no canonical sorted-key JSON writer exists in the control-plane today. | Proposal can still over-promise deterministic JSON without naming a real owner. | Live blocker proof. |
| MAP-06 | `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift` | Swift runtime | stable dossier fallback + link-role behavior | 2026-04-15 | High | Stable V1 builds dossiers for implicated runs or the first five observation runs when none are implicated, and persists non-implicated observation runs as `context` links. | Proposal can still under-port evidence completeness while claiming parity. | Live blocker proof. |
| MAP-07 | `control-plane/crates/mcp-server/src/tools/mod.rs` and `control-plane/crates/mcp-server/src/server.rs` | Rust MCP | current tool-dispatch contract | 2026-04-15 | High | Current control-plane still exposes only namespaced tool modules through `tools/list` / `tools/call`. | Prevents replaying the old raw-method blocker. | Closure proof. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | daemon-global Steward config/current-catalog owner chain | Baseline + current repo | 2026-04-15 | High | Current daemon has no startup or northbound owner for current `StewardConfig` or current `AgentCatalogFile` outside per-run YAML paths. | Proposal still assumes those parsed objects exist for startup hash checks and manual runs. | Live blocker. |
| INT-02 | deterministic JSON artifact ownership | Targeted refresh | 2026-04-15 | High | Current control-plane has no canonical sorted-key JSON writer; plain `serde_json` is the active artifact-writing pattern. | Proposal still over-promises byte-stable JSON with `HashMap`-heavy structs. | Live blocker. |
| INT-03 | dossier fallback / link-role parity | Baseline + current repo | 2026-04-15 | High | Stable V1 still emits context dossiers and `context` run links when there are no implicated runs. | Proposal still narrows evidence completeness to implicated-only dossiers. | Live blocker. |
| INT-04 | config hashing owner chain | Baseline + current repo | 2026-04-15 | High | The proposal’s old raw-hash contradiction is closed and aligned with canonical parsed-object hashing. | No current blocker here. | Closure note. |
| INT-05 | manual MCP trigger owner | Baseline + current repo | 2026-04-15 | High | The proposal’s old raw-method contradiction is closed and aligned with current MCP tool ownership. | No current blocker here. | Closure note. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Optional `system_steward` lane | Specified | DOC-01, DOC-03 | `StewardAnalysisService`, current ACP execution path | Old omission finding is closed. |
| Optional `steward_auditor` dependency semantics | Specified | DOC-01, DOC-03 | `StewardAnalysisService` | Old dependency blocker is closed. |
| Full nested strategy-profile validation parity | Specified | DOC-01, DOC-05 | stable validator contract | Old validation blocker is closed. |
| Config-change dual-hash semantics | Partial | DOC-01, DOC-05, INT-01, INT-04 | startup hashing and current-catalog resolution | Hash semantics are now correct, but the owner of the hashed objects is still missing. |
| Manual Steward MCP trigger | Partial | DOC-01, DOC-04, MAP-07 | current MCP tool surface | Tool ownership is correct, but current config/catalog sourcing for the triggered analysis is still unspecified. |
| Deterministic JSON artifacts | Contradicted by repo | DOC-01, MAP-05, INT-02 | artifact writer + proposal `HashMap` model | Proposal still lacks an executable determinism owner. |
| No-signal dossier fallback | Contradicted by repo | DOC-01, DOC-03, MAP-06, INT-03 | Swift Steward V1 | Proposal still narrows dossiers to implicated-only runs. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal acceptance | deterministic rerun and dossier parity | current acceptance asserts identical reruns and dossier contents, but not the deterministic owner chain or no-signal context fallback explicitly enough | acceptance should explicitly bind deterministic map/container ownership and no-signal context-dossier behavior | 2026-04-15 | High | Current AC can still pass with underspecified JSON ordering and missing context dossiers. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | daemon startup / current config sources | `§4e` / `§6b` assume current parsed `StewardConfig` and current parsed `AgentCatalogFile` are available for startup hashing and manual runs | current daemon startup still has no Steward-specific config/catalog owner; start surfaces only carry per-run YAML paths | 2026-04-15 | High | One high-severity input-owner gap remains. |
| REAL-02 | deterministic JSON contract | `§7` and AC-7 promise sorted deterministic JSON artifacts with identical reruns | current proposal still models artifact-visible collections as `HashMap` and names only plain `serde_json`; current daemon uses `serde_json::to_string_pretty(...)` | 2026-04-15 | High | One high-severity determinism gap remains. |
| REAL-03 | evidence dossier completeness | proposal narrows dossiers to implicated runs | stable V1 still falls back to the first five observation runs and persists them as `context` links when there are no implicated runs | 2026-04-15 | High | One medium parity gap remains. |
| REAL-04 | old default-trigger blocker | older review basis said the draft still flipped `post_run_hook.enabled` to `true` | current proposal now correctly keeps `post_run_hook.enabled = false` | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-05 | old config-hash blocker | older review basis said the draft still hashed raw YAML file content | current proposal now explicitly uses canonical parsed-object hashing | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-06 | old manual-trigger blocker | older review basis said the draft still used a raw `steward/run_analysis` method | current proposal now defines `steward.run_analysis` on the existing namespaced MCP tool surface | 2026-04-15 | High | Earlier blocker is stale. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-03 | The missing daemon Steward slice is still real. |
| Scope boundaries | Specified | DOC-01 | Scope is now close to stable Steward V1 parity. |
| Reusable baseline coverage | Partial | BASE-02, BASE-03, REAL-01, REAL-02, REAL-03 | Most old gaps are closed; three live seams remain. |
| Data / execution contract | Partial | INT-01, INT-02, REAL-01, REAL-02 | Config/catalog sourcing and deterministic JSON ownership are still underspecified. |
| Config / validation contract | Partial | DOC-05, INT-01, REAL-01, REAL-05 | Hash semantics are correct, but the hashed-object owner chain is still missing. |
| Testing strategy | Partial | TEST-01 | Acceptance should explicitly bind deterministic JSON ownership and context-dossier fallback. |
| Dependencies / integration points | Partial | MAP-01, MAP-02, MAP-03, MAP-04, MAP-07 | MCP ownership is correct, but current daemon inputs are still incomplete. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P049` still intends parity with stable Steward V1 evidence completeness, including context dossiers when there are no implicated runs.
- ASSUMP-02: `P049` still intends byte-stable deterministic JSON, not merely semantically equivalent JSON.
- QUESTION-01: Where should the daemon get the current `StewardConfig` and current `AgentCatalogFile` for startup hash checks and manual Steward runs?
- QUESTION-02: Does the author want deterministic JSON by deterministic container choice (`BTreeMap` / sorted arrays) or by a dedicated canonical writer utility?
- BLOCKER-01: the proposal still assumes daemon-global config/catalog inputs that the current daemon does not yet own.
- BLOCKER-02: the proposal still promises deterministic sorted-key JSON without naming a deterministic map/serializer owner.

## L. Research Triggers / External Questions
No external research trigger is required. Local proposal/docs/code/baseline evidence are sufficient for a readiness verdict.
