# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/049-steward-analysis-system.md` | 2026-04-15 | High | The current draft already closes the stale blockers from the previous round: it now carries an end-to-end `project_key` owner chain, materializes the active steward input artifacts (`agent_catalog_snapshot`, `workflow_snapshot`, `config_change_log`), includes `context_strategy_profiles` in config hashing, and defines explicit GraphQL/MCP/resource readback. Two live gaps remain: the frozen run-start owner chain is still incompatible with current active run-start ingress paths, and the proposal never defines how pre-P049 runs without the new frozen fields participate in Steward analysis. | A stale review would keep already-closed blockers open and miss the real remaining readiness risks. | Primary reviewed artifact. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should start from the current stable repo baseline rather than superseded proposal history. | Review could drift from the current system shape. | Reusable intake baseline. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-15 | High | Steward is already a stable product boundary in the current system baseline. | Review could misjudge the proposal as greenfield. | Product/system framing. |
| DOC-04 | `docs/reference/forge-steward.md` | 2026-04-15 | High | Stable Steward V1 still analyzes completed runs from persisted truth, groups by primary cohort, and relies on historical windows. | Proposal could ignore migration/read semantics for existing runs. | Primary parity anchor. |
| DOC-05 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current daemon northbound start surfaces are still GraphQL `startRun` and MCP `runs.start`. | Proposal must stay executable against those live run-start seams. | Current host-system boundary. |
| DOC-06 | `docs/reference/context-strategy-and-experiment-framework.md` | 2026-04-15 | High | `context_strategy_profiles` is already a stable config slice. | Review could replay the already-closed hash-scope blocker. | Freshness control. |
| DOC-07 | `examples/steward/steward_config.yaml` | 2026-04-15 | High | Live steward config includes `context_strategy_profiles`, and the proposal now accounts for that explicitly. | A stale review would keep a closed blocker open. | Freshness control. |
| DOC-08 | `examples/agents/agents.yaml` | 2026-04-15 | High | The active steward catalog still requires six `system_steward` inputs and one dependent `steward_auditor` input chain, and the current draft now reflects them. | Review could replay an already-closed input-parity blocker. | Active catalog contract. |
| DOC-09 | Existing `docs/proposals/049-steward-analysis-system.review/*` artifacts | 2026-04-15 | High | The current local review artifacts are stale against the updated draft and still center on blockers the proposal now addresses. | Review output would misstate readiness unless refreshed. | Freshness boundary. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-level review posture | 2026-04-15 | High | Fresh for intake. | Orientation. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current subsystem boundaries | 2026-04-15 | High | Fresh for product/system framing. | Host-system expectations. |
| BASE-03 | `docs/reference/forge-steward.md` | Reused | stable Steward V1 semantics | 2026-04-15 | High | Fresh for deterministic pipeline, cohorting, and trigger parity. | Primary parity baseline. |
| BASE-04 | `docs/reference/rust-control-plane.md` | Reused | live GraphQL/MCP northbound seams | 2026-04-15 | High | Fresh for current run-start surfaces. | Run-start compatibility. |
| BASE-05 | prior `049` review artifacts | Partially refreshed | stale findings only | 2026-04-15 | High | Reused only to identify which old blockers the current draft already closed. | Freshness control. |
| BASE-06 | `docs/proposals/049-steward-analysis-system.review/integration-context.md` | Missing | proposal-local reusable context | 2026-04-15 | High | No separate integration-context file exists. The affected surfaces were narrow enough to refresh directly. | Not a blocker. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - deterministic Rust Steward pipeline,
  - run-owned cohort and provenance freezing,
  - daemon-owned current inputs,
  - queue triggers,
  - optional steward agent lanes,
  - northbound analysis readback.
- Out of scope:
  - dedicated Steward dashboard UI,
  - schedule-trigger wiring,
  - V2 recommendation synthesis,
  - V3 experiment execution,
  - live-session introspection outside persisted truth.
- Closed stale blockers in the current draft:
  - `project_key` is now wired through domain, repo, migration, and `ideas.create`,
  - active-catalog steward input materialization is now explicitly defined,
  - `context_strategy_profiles` is now explicitly included in config hashing,
  - GraphQL/MCP/resource readback is now explicit,
  - migration numbering is no longer stale.
- Open questions:
  - Should all run-start surfaces be forced to provide workflow/catalog YAML paths, or should Steward simply exclude non-YAML-backed runs?
  - Should pre-P049 runs be excluded from cohorts, backfilled once, or mapped through an explicit legacy fallback policy?
- Current live blockers discovered in this round:
  - the frozen run-start owner chain is not yet reconciled with the current active GraphQL and optional MCP run-start ingress,
  - the proposal never defines eligibility/migration rules for existing completed runs that predate the new frozen fields.

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | GraphQL `startRun` mutation | Targeted refresh | 2026-04-15 | High | Current GraphQL run start still hardcodes `workflow_yaml_path: None` and `agent_catalog_yaml_path: None`. | The proposal can require compiler-derived frozen metadata at run creation but still leave one active run-start surface unable to provide the inputs. | Live blocker. |
| NAV-02 | MCP `runs.start` tool | Targeted refresh | 2026-04-15 | High | Current MCP run start accepts `workflow_yaml_path` and `agent_catalog_yaml_path`, but both remain optional. | The proposal can leave callers able to create runs without the inputs required by its frozen metadata contract. | Live blocker. |
| NAV-03 | `StartRun` / compiler bridge | Proposal + targeted refresh | 2026-04-15 | High | The proposal correctly puts frozen metadata and snapshot production on run creation via compiler output. | This owner chain only works if run-start ingress is updated to supply the required inputs. | Core architecture seam. |
| NAV-04 | Steward analysis over completed runs | Proposal + baseline | 2026-04-15 | High | Stable Steward analysis still works over completed persisted runs and historical windows. | Without a legacy-run rule, existing runs with null frozen fields have no defined cohorting behavior. | Historical dataset semantics. |
| NAV-05 | GraphQL/MCP/resource readback | Proposal + baseline | 2026-04-15 | High | The current draft now names explicit GraphQL queries, MCP tools, and `steward-analysis://` readback. | Review could incorrectly preserve the stale readback blocker. | Closed blocker. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/domain/src/commands.rs` | Domain command contract | `StartRunCmd` ingress payload | 2026-04-15 | High | `workflow_yaml_path` and `agent_catalog_yaml_path` are optional on the canonical run-start command today. | The proposal’s owner chain cannot rely on them without updating the command contract or explicitly constraining eligible runs. | Live blocker. |
| MAP-02 | `control-plane/crates/graphql-server/src/schema.rs` | GraphQL | active `startRun` mutation | 2026-04-15 | High | GraphQL `start_run` still hardcodes both YAML paths to `None`. | One active northbound run-start surface cannot satisfy the proposal’s frozen snapshot contract. | Live blocker. |
| MAP-03 | `control-plane/crates/mcp-server/src/tools/runs.rs` | MCP | active `runs.start` tool | 2026-04-15 | High | MCP exposes both YAML paths, but leaves them optional. | Steward-compatible run creation remains under-specified for current callers. | Live blocker. |
| MAP-04 | `control-plane/crates/workflow/src/definition.rs` | Workflow schema | workflow metadata owner | 2026-04-15 | High | The proposal correctly widens workflow metadata with `family`, `risk_class`, and `stack`. | This closed the stale workflow-owner blocker. | Freshness control. |
| MAP-05 | `control-plane/crates/engine/src/command_handler.rs` | Engine | `StartRun` persistence owner | 2026-04-15 | High | `StartRun` is still the right owner for freezing run-owned metadata and snapshot hashes. | The upstream inputs must reach this seam consistently. | Core owner boundary. |
| MAP-06 | `control-plane/crates/domain/src/run.rs` | Domain aggregate | run-owned frozen fields | 2026-04-15 | High | The proposal correctly widens `Run` with cohort, snapshot, and drift fields. | Existing pre-P049 rows still need an explicit analysis policy. | Historical-run blocker. |
| MAP-07 | `control-plane/crates/db/src/repos/ideas.rs` and `mcp-server/src/tools/ideas.rs` | Persistence / ingress | `project_key` owner chain | 2026-04-15 | High | The current draft now names the full `project_key` chain end-to-end. | The old `project_key` blocker is stale. | Closed blocker. |
| MAP-08 | `examples/agents/agents.yaml` | Catalog | active steward IO contract | 2026-04-15 | High | The proposal now matches the live steward input and output vocabulary. | The old input-parity blocker is stale. | Closed blocker. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Frozen workflow metadata at run creation | Proposal section 3a, `domain/src/commands.rs`, `graphql-server/src/schema.rs`, `mcp-server/src/tools/runs.rs` | Ingress / write | 2026-04-15 | High | The proposal’s canonical owner chain depends on workflow/catalog YAML paths at run creation, but current active ingress paths do not guarantee them. | New runs can still be created without the inputs needed to satisfy the proposal’s own frozen metadata contract. | Critical gap. |
| DATA-02 | Frozen snapshot provenance | Proposal section 3a, `engine/src/command_handler.rs` | Ingress / write | 2026-04-15 | High | Snapshot hashes and payloads are correctly owned by the compiler-to-StartRun bridge in the proposal. | That bridge remains incomplete unless ingress surfaces are updated too. | Critical gap. |
| DATA-03 | Historical run eligibility | Proposal pipeline section 2a, run widening in section 3a, stable Steward reference | Read / cohorting | 2026-04-15 | High | The proposal never states what Steward does with existing runs that predate the new frozen fields. | Historical windows can become nondeterministic or silently shrink without a declared rule. | High-severity gap. |
| DATA-04 | `project_key` owner chain | Proposal section 3a, `ideas.rs`, `ideas.create` | Read/write | 2026-04-15 | High | The current draft now explicitly widens domain, repo, migration, and ingress for `project_key`. | The old `project_key` blocker is closed and should not survive. | Freshness control. |
| DATA-05 | Active steward input materialization | Proposal section 3c, section 9, `examples/agents/agents.yaml` | Read artifact materialization | 2026-04-15 | High | The current draft now names owners, canonical paths, and artifact IDs for `agent_catalog_snapshot`, `workflow_snapshot`, and `config_change_log`. | The old input-materialization blocker is closed and should not survive. | Freshness control. |
| DATA-06 | Config-change hash scope | Proposal section 7b, `examples/steward/steward_config.yaml`, `docs/reference/context-strategy-and-experiment-framework.md` | Read at startup | 2026-04-15 | High | The proposal now explicitly includes `context_strategy_profiles` in the parsed-object hash. | The old hash-scope blocker is closed and should not survive. | Freshness control. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Run-start ingress boundary | Current repo + proposal | 2026-04-15 | High | The proposal freezes new run-owned truth at `StartRun`, but current northbound start surfaces do not yet provide required inputs consistently. | Frozen metadata ownership is still incomplete at the ingress edge. | Critical blocker. |
| INT-02 | Historical completed-run dataset | Baseline + proposal | 2026-04-15 | High | Steward remains a historical observer over completed runs, but the proposal defines no legacy-run policy. | Existing DB truth can be under-specified for analysis eligibility. | High blocker. |
| INT-03 | `project_key` owner chain | Proposal + current repo | 2026-04-15 | High | The current draft now names the correct idea-domain, repo, migration, and MCP ingress seam. | This older blocker is stale. | Closed blocker. |
| INT-04 | Active steward contract parity | Proposal + current repo | 2026-04-15 | High | The current draft now materializes the live steward input contract instead of only aligning outputs. | This older blocker is stale. | Closed blocker. |
| INT-05 | GraphQL/MCP/resource readback | Proposal + baseline | 2026-04-15 | High | Named readback surfaces are now explicit. | The earlier readback blocker is stale. | Closed blocker. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Partial | NAV-01, NAV-02, DATA-01, DATA-02, INT-01 | GraphQL `startRun`, MCP `runs.start`, `StartRun` | Core owner chain is defined, but active ingress surfaces are not yet reconciled. |
| Happy path | Specified | DOC-01, DOC-04 | deterministic Steward pipeline | Core observer flow is explicit and materially stronger than the stale review basis. |
| No-signal analysis | Specified | DOC-01, DOC-04 | dossiers + inconclusive status | Earlier no-signal blocker remains closed. |
| Optional LLM lane | Specified | DOC-01, DOC-08, DATA-05 | steward input/output artifact materialization | Current-catalog parity is now explicitly described. |
| Config-change pending flag | Specified | DOC-01, DOC-06, DOC-07, DATA-06 | daemon runtime inputs and hashing | Earlier hash-scope blocker is closed. |
| Historical completed runs | Partial | DOC-04, DATA-03, INT-02 | persisted runs dataset | Legacy/pre-P049 run behavior is still undefined. |
| Northbound readback | Specified | DOC-01, NAV-05, INT-05 | GraphQL, MCP, resource lane | Earlier readback blocker is closed. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | Steward daemon slice | Standard proposal sequencing | Standard non-landing / revert | 2026-04-15 | Medium | No new rollout blocker surfaced. Remaining issues are owner-chain and historical-data semantics. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | Deterministic metrics + recommendations | cross-run health analysis | Steward analysis execution | 2026-04-15 | High | Metric-source and active-catalog parity claims are now materially stronger than in the stale review basis. |
| METRIC-02 | Config-change pending flag | explain why analysis ran | daemon startup + next completed run | 2026-04-15 | High | Hash-scope ambiguity is now closed at proposal level. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal-specific gate | owner freeze, cohort parity, bootstrap semantics, determinism, parity, readback | Proposal now defines a focused `proposal-049|p049` gate | The gate still needs explicit coverage for active GraphQL/MCP run-start eligibility or rejection once the ingress rule is chosen | 2026-04-15 | High | Proof lane should catch the live ingress blocker after the proposal is corrected. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | `project_key` owner chain | Current draft widens `Idea`, repo, migration, and `ideas.create` | Proposal text now names all those owners explicitly | 2026-04-15 | High | The old `project_key` blocker is closed and must be removed from the review. |
| REAL-02 | Active steward input parity | Current draft chooses current-catalog parity | Proposal now materializes `agent_catalog_snapshot`, `workflow_snapshot`, and `config_change_log` with canonical paths | 2026-04-15 | High | The old input-materialization blocker is closed and must be removed from the review. |
| REAL-03 | Config hash scope | Current draft hashes the full effective parsed `StewardConfig` including `context_strategy_profiles` | Proposal now states that inclusion explicitly | 2026-04-15 | High | The old hash-scope blocker is closed and must be removed from the review. |
| REAL-04 | Active run-start ingress | Proposal requires compiler-derived frozen metadata and snapshot hashes at `StartRun` | Current GraphQL `startRun` still passes no YAML/catalog paths, and current MCP `runs.start` still leaves them optional | 2026-04-15 | High | The proposal’s new frozen-truth contract is not yet executable across active ingress surfaces. |
| REAL-05 | Historical completed runs | Proposal reads completed runs from DB and forbids recomputation from mutable files during analysis | Existing runs in current SQLite truth can predate the new frozen fields and have no declared backfill/exclusion policy | 2026-04-15 | High | Analysis eligibility for historical runs remains under-specified. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, DOC-03, DOC-04 | Missing Rust Steward slice remains a real need. |
| Scope boundaries | Specified | DOC-01 | Scope remains appropriately bounded. |
| Reusable baseline coverage | Specified | DOC-02, DOC-03, DOC-04, DOC-05 | Current draft is now much closer to stable references than the stale review suggested. |
| Navigation / entry points | Partial | NAV-01, NAV-02, NAV-03, NAV-04 | Main seams are named, but active run-start compatibility is still unresolved. |
| State handling | Partial | H matrix | Core states are strong; historical-run handling remains incomplete. |
| Data / API contract | Partial | DATA-01, DATA-02, DATA-03, REAL-04, REAL-05 | Active ingress and legacy-run behavior still require one more contract pass. |
| Persistence / caching | Partial | MAP-05, MAP-06, DATA-03 | New tables and fields are clear, but historical row semantics are not. |
| Feature flags / rollout / rollback | Specified | FLAG-01 | No new rollout blocker found. |
| Analytics / instrumentation | Specified | METRIC-01, METRIC-02 | Stronger than the stale review basis. |
| Testing strategy | Partial | TEST-01 | Focused proof lane is correct, but it should cover the chosen ingress/legacy-run rule. |
| Dependencies / integration points | Partial | INT-01, INT-02 | Run-start ingress and historical dataset policy remain incomplete. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Proposal 049 remains a delta on the stable Steward V1 reference rather than a replacement for that reference.
- ASSUMP-02: Active GraphQL `startRun` and MCP `runs.start` remain live northbound run-start surfaces unless the proposal explicitly narrows or replaces them.
- QUESTION-01: Should P049 require workflow/catalog YAML paths on every run-start surface, or explicitly declare non-YAML-backed runs ineligible for Steward?
- QUESTION-02: For pre-P049 runs that lack the new frozen fields, is the intended rule exclusion, one-time backfill, or a bounded legacy fallback set?
- BLOCKER-01: The frozen run-start owner chain is not yet reconciled with current active GraphQL and MCP run-start ingress paths.
- BLOCKER-02: The proposal never defines how pre-P049 completed runs without the new frozen fields participate in Steward analysis.

## O. Research Triggers / External Questions
No external research triggers were required. Local proposal, stable references, active catalog/config, and current control-plane code were sufficient for a defensible proposal-readiness call.
