# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/046-structured-output-envelope-and-contract-validation.md` | 2026-04-15 | High | The current draft now explicitly closes the previous proposal-blocking seams: it restores full contract binding, makes validation-failure persistence attempt-aware, fixes the typed northbound payload source to `validation_failure_records.record_json`, and names `workflow/src/catalog.rs` as the `human_format` parser owner. | Review could replay stale blockers that no longer exist in the draft. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should still anchor to the current stable repo baseline before judging daemon work. | Review could ignore already-stable host-system contracts. | Intake baseline. |
| DOC-03 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-15 | High | Stable contract truth for this slice requires explicit contract binding, full schema fields including `human_format`, stage-owned failure evidence, and same-run retry lineage. | Proposal could still drift if it only partially ports the stable slice. | Main stable-reference anchor. |
| DOC-04 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-15 | High | Stage truth remains attempt-precise and stage-owned; recovery and report surfaces must read canonical stage truth rather than infer it from loose artifacts. | Proposal could still flatten or duplicate recovery truth. | Stage / retry authority. |
| DOC-05 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current daemon northbound reads are still projection-backed GraphQL plus MCP resource/tool surfaces. | Proposal must map validation-failure surfacing onto those real reader seams. | Current daemon boundary anchor. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake baseline | 2026-04-15 | High | Fresh enough as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | Partially refreshed | stable contract / failure-evidence / retry-lineage owner chain | 2026-04-15 | High | Refreshed narrowly against current Swift and Rust code paths. | Stable-reference authority. |
| BASE-03 | `docs/reference/execution-truth-and-recovery.md` | Partially refreshed | stage-owned truth and recovery ownership | 2026-04-15 | High | Refreshed narrowly against current attempt-aware stage model. | Retry-lineage baseline. |
| BASE-04 | `docs/reference/rust-control-plane.md` | Partially refreshed | daemon GraphQL / MCP reader boundary | 2026-04-15 | High | Refreshed narrowly against current code and the working-tree diff in the relevant Rust reader files. | Current control-plane baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - envelope extraction from ACP responses
  - post-generation validation using the stable owner chain
  - durable validation-failure persistence
  - canonical metadata/path binding before persistence
  - stage/artifact surfacing of validation-failure truth
- Out of scope:
  - broader thin-client cutover
  - unrelated release / approval proposals
  - runtime gate execution
- Assumptions:
  - review mode is `proposal-readiness`
  - the stable output-contract reference remains authoritative for this slice
  - the review should judge the current working-tree draft rather than reuse stale prior findings
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | stable report / recovery readers | Baseline + current repo + proposal | 2026-04-15 | High | The draft now keeps failure truth stage-owned and attempt-aware, matching the stable operator/report surfaces. | Review could miss a hidden retry-lineage contradiction. | Failure-truth continuity. |
| NAV-02 | daemon GraphQL / MCP readers | Baseline + current repo + proposal | 2026-04-15 | High | The draft now binds typed payload delivery to the existing readers using `validation_failure_records.record_json` as the source of truth. | Review could incorrectly keep a stale northbound-owner blocker open. | Northbound relevance. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/db/migrations/002_projections.sql` | Rust DB | current stage projection identity | 2026-04-15 | High | `stage_summaries` is keyed by `stage_execution_id`, not by logical `stage_id` alone. | Any proposal that flattens validation-failure truth by logical stage would still contradict repo truth. | Retry-lineage proof. |
| MAP-02 | `control-plane/crates/db/src/repos/projections.rs` | Rust DB / projections | current stage summary and artifact projection reads | 2026-04-15 | High | Stage reads are execution-row based, artifact reads are projection-backed, and both are the actual current reader seams the proposal must wire into. | Review could miss a real owner-chain omission. | Projection and reader proof. |
| MAP-03 | `Chainworks Forge/Models/StageExecution.swift` and `Chainworks Forge/Models/AgentExecution.swift` | Swift stable model | stable attempt-aware failure truth | 2026-04-15 | High | Stable truth remains tied to stage/agent execution attempts, including validation-failure JSON on attempt-owned records. | Proposal could still lose attempt identity in the daemon port. | Stable parity proof. |
| MAP-04 | `control-plane/crates/graphql-server/src/types/artifact.rs` and `control-plane/crates/graphql-server/src/schema.rs` | Rust GraphQL | current artifact reader payload shape | 2026-04-15 | High | Current GraphQL artifact reads are metadata/projection-backed today, which is why the proposal must explicitly wire typed payload loading onto this seam. | Without explicit proposal owner-chain text, typed payload delivery would remain underspecified. | Northbound proof. |
| MAP-05 | `control-plane/crates/mcp-server/src/tools/reports.rs` | Rust MCP tool | current report-artifact tool path | 2026-04-15 | High | `reports.get` already filters report artifacts and is the correct MCP tool owner for decoded validation-failure payload delivery. | Proposal could still name the wrong northbound surface. | Northbound proof. |
| MAP-06 | `control-plane/crates/mcp-server/src/server.rs` and `control-plane/crates/mcp-server/src/tools/stages.rs` | Rust MCP resources / tools | current `report://{run_id}` and stage read path | 2026-04-15 | High | `report://` is the existing report resource path, and `stages.retry` is command-only; there is no competing stage read tool. | Proposal could still invent the wrong read surface. | Northbound proof. |
| MAP-07 | `control-plane/crates/workflow/src/catalog.rs` | Rust workflow parser | current contract schema ingestion | 2026-04-15 | High | Current `ContractDef` still lacks `human_format`, which is exactly why the proposal must name `catalog.rs` as the parser owner. | Review could miss whether the draft actually fixes the parser-owner gap. | Schema-parity proof. |
| MAP-08 | `control-plane/crates/workflow/src/plan.rs` and `control-plane/crates/workflow/src/compiler.rs` | Rust workflow compiler | current output-schema carrier | 2026-04-15 | High | Current compiler/plan own contract propagation into compiled tasks, so they are the right downstream owners once the parser is extended. | Proposal could under-specify full schema propagation. | Schema-parity proof. |
| MAP-09 | current working-tree diff in `control-plane/crates/mcp-server/src/tools/reports.rs` and `control-plane/crates/mcp-server/src/server.rs` | Rust MCP readers | current draft-only northbound changes | 2026-04-15 | High | The active draft widens release-artifact filtering and canonical run reads, but it still does not decode or surface `ValidationFailureRecord` payloads. | Review could incorrectly assume the northbound payload seam was already solved in code. | Current-draft freshness proof. |
| MAP-10 | current working-tree diff in `control-plane/crates/workflow/src/catalog.rs` and `control-plane/crates/workflow/src/plan.rs` | Rust workflow parser/compiler | current draft-only workflow changes | 2026-04-15 | High | The active draft adds skill/worktree metadata, but it still does not carry `human_format` through Rust `OutputSchema`. | Review could incorrectly assume the schema-parity seam was already solved in code. | Current-draft freshness proof. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | same-run retry lineage for validation failures | Baseline + current repo + proposal | 2026-04-15 | High | The draft now aligns failure persistence and projection derivation to exact execution identity. | No live proposal-blocking conflict remains on retry lineage. | Former blocker now closed. |
| INT-02 | typed northbound failure-record delivery | Baseline + current repo + proposal | 2026-04-15 | High | The draft now assigns one authoritative payload source and names the real current reader owners. | No live proposal-blocking conflict remains on northbound typed payload delivery. | Former blocker now closed. |
| INT-03 | full contract-schema parity | Baseline + current repo + proposal | 2026-04-15 | High | The draft now includes `human_format` at the parser, compiler, and plan layers. | No live proposal-blocking conflict remains on schema completeness. | Former blocker now closed. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Envelope extraction from ACP response text | Specified | DOC-01 | Rust ACP / executor seam | Direction is concrete and grounded. |
| Full resolver-chain parity including explicit `output_contract` | Specified | DOC-01, DOC-03 | compiler and execution binding | Earlier blocker remains closed. |
| `structured_with_human_companion` companion requirement | Specified | DOC-01, DOC-03 | stable contract-mode semantics | Earlier blocker remains closed. |
| Durable `ValidationFailureRecord` continuity fields | Specified | DOC-01, DOC-03 | failure-evidence record | Stable continuity fields remain explicit. |
| Attempt-aware validation-failure truth across retries | Specified | DOC-01, DOC-03, DOC-04, MAP-01, MAP-02, MAP-03, INT-01 | DB schema + stage projections + stable attempt model | Draft now matches repo and stable retry lineage. |
| Decoded validation-failure payload on current northbound readers | Specified | DOC-01, DOC-05, MAP-04, MAP-05, MAP-06, INT-02 | GraphQL / MCP reader path | Draft now fixes the payload source and owner chain. |
| `human_format` ingestion from catalog | Specified | DOC-01, DOC-03, MAP-07, MAP-08, INT-03 | Rust catalog parser + compiler | Draft now closes the parser-owner gap. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | retry-lineage truth | validation failures across same-run retries | proposal now states the needed attempt-aware row and exact projection derivation | implementation should prove failed-first-attempt then successful-retry isolation explicitly | 2026-04-15 | High | Residual implementation watchpoint, not a proposal gap. |
| TEST-02 | northbound typed payload delivery | GraphQL / MCP failure-record payload | proposal now fixes the typed source and current reader owners | implementation should prove decoded payload delivery on `GqlArtifact`, `reports.get`, and `report://{run_id}` | 2026-04-15 | High | Residual implementation watchpoint, not a proposal gap. |
| TEST-03 | schema completeness | `human_format` ingestion | proposal now names parser, compiler, and plan owners | implementation should prove `human_format` survives YAML parse through compiled schema | 2026-04-15 | High | Residual implementation watchpoint, not a proposal gap. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | validation-failure storage / projection join | draft now keys rows with execution identity and derives stage flags by exact `stage_execution_id` | current stage projection and stable retry truth are execution-attempt-aware | 2026-04-15 | High | Earlier retry-lineage blocker is closed. |
| REAL-02 | typed failure-record delivery | draft now uses `validation_failure_records.record_json` as the authoritative typed payload source for current readers | current readers are metadata/projection-backed today and therefore need exactly this explicit owner chain | 2026-04-15 | High | Earlier northbound-owner blocker is closed. |
| REAL-03 | schema completeness | draft now extends `human_format` through parser, compiler, and plan and names `workflow/src/catalog.rs` explicitly | current Rust parser still lacks `human_format`, which is why this owner must be in the proposal | 2026-04-15 | High | Earlier schema-parity blocker is closed. |
| REAL-04 | old resolver blocker | draft preserves explicit `output_contract` parity | stable resolver-chain requirement still stands | 2026-04-15 | High | Earlier blocker remains closed. |
| REAL-05 | old companion blocker | draft preserves the requirement that `structured_with_human_companion` must persist both artifacts | stable companion rule still stands | 2026-04-15 | High | Earlier blocker remains closed. |
| REAL-06 | fresh MCP working-tree edits | proposal still assigns northbound validation-failure delivery to existing readers | current draft-only edits in `reports.rs` / `server.rs` widen release and canonical-run surfaces but do not add decoded validation-failure payload delivery | 2026-04-15 | High | No new contradiction was introduced; the proposal still targets a live unsolved seam. |
| REAL-07 | fresh workflow working-tree edits | proposal still assigns `human_format` ingestion to parser/compiler/plan owners | current draft-only edits in `catalog.rs` / `plan.rs` add skill/worktree metadata but do not solve `human_format` propagation | 2026-04-15 | High | No new contradiction was introduced; the proposal still targets a live unsolved seam. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Envelope parsing and post-generation validation remain real daemon gaps. |
| Scope boundaries | Specified | DOC-01 | Scope is clear and disciplined. |
| Reusable baseline coverage | Specified | BASE-02, BASE-03, BASE-04, INT-01, INT-02, INT-03 | Stable references and current repo seams are reflected in the draft. |
| Data / execution contract | Specified | DOC-01, DOC-03, INT-01 | Validation-failure persistence now keeps attempt identity. |
| Failure / recovery semantics | Specified | DOC-03, DOC-04, INT-01, INT-02 | Durable evidence and current reader delivery are both explicitly covered. |
| Testing strategy | Specified | TEST-01, TEST-02, TEST-03 | Acceptance criteria cover the critical seams; implementation still needs to realize them. |
| Dependencies / integration points | Specified | DOC-05, MAP-04, MAP-05, MAP-06, MAP-07, MAP-08 | Reader and parser owners are now named concretely enough to implement. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P046` still intends stable parity for same-run retry / inspectable failure history, not just one-shot validation on first attempt.
- ASSUMP-02: Typed validation-failure payload delivery should stay within the current GraphQL / MCP boundary rather than adding a new northbound tool.
- QUESTION-01: No blocking proposal question remains for the next round. Loader and batching details can be resolved during implementation.
- BLOCKER-01: None.

## L. Research Triggers / External Questions
No external research trigger was required. Local proposal/docs/code/baseline evidence are sufficient for a proposal-readiness verdict.
