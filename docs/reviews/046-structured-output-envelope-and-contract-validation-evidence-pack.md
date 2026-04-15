# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/046-structured-output-envelope-and-contract-validation.md` | 2026-04-14 | High | The current draft has materially improved: it now carries `receipt_exists`, `transcript_exists`, raw/normalized artifact identity, projection-backed `has_validation_failure`, and explicit MCP resource naming. Three live gaps remain: compiler resolver parity, companion-mode semantics, and full-record northbound delivery. | Review could repeat stale blockers or miss the remaining proposal-text gaps. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-14 | High | Proposal review should anchor to the current stable repo baseline before judging daemon work. | Review could ignore already-stable host-system contracts. | Intake baseline. |
| DOC-03 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-14 | High | The stable reference already fixes the owner chain for contract validation, durable failure evidence, and structured-output mode semantics, including the “persist both artifacts” rule for `structured_with_human_companion`. | Proposal can still drift if it ports only part of the stable contract slice. | Main stable-reference anchor. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-14 | High | Current daemon northbound reads are projection-backed GraphQL plus MCP resources; current report/resource readers are metadata-oriented, not full decoded failure-record readers. | Proposal can overclaim northbound parity without actually binding the payload path. | Current daemon boundary anchor. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake baseline | 2026-04-14 | High | Fresh enough as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | Partially refreshed | stable contract / validation / recovery owner chain | 2026-04-14 | High | Refreshed narrowly against current Swift code paths. | Stable-reference authority. |
| BASE-03 | `docs/reference/rust-control-plane.md` | Partially refreshed | daemon GraphQL / MCP reader boundary | 2026-04-14 | High | Refreshed narrowly against current code. | Current control-plane baseline. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - envelope extraction from ACP responses
  - post-generation validation using the stable owner chain
  - durable validation-failure persistence
  - canonical metadata/path binding before persistence
  - stage/artifact surfacing of validation failure truth
- Out of scope:
  - broader thin-client cutover
  - unrelated release / approval proposals
  - runtime gate execution
- Assumptions:
  - review mode is `proposal-readiness`
  - the stable output-contract reference remains authoritative for this slice
  - the user wants a current-head reread rather than reuse of the previous review basis
- Blockers:
  - the draft still does not say how Rust compilation preserves the full current contract resolver chain
  - the draft still weakens `structured_with_human_companion`
  - the draft still does not assign a concrete northbound payload path for decoded validation-failure truth

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | stable report / recovery readers | Baseline + current repo | 2026-04-14 | High | Current report and recovery readers decode `ValidationFailureRecord` directly, not just booleans or artifact presence. | Review could miss a metadata-only northbound drift. | Failure-truth continuity. |
| NAV-02 | daemon GraphQL / MCP readers | Baseline + current repo | 2026-04-14 | High | Current daemon readers expose stage summaries and artifact metadata, but not the decoded validation-failure payload. | Proposal can overclaim parity while still leaving readers blind to details. | Northbound relevance. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/Engine/OutputContractResolverV2.swift` | Swift runtime | canonical contract-resolution owner | 2026-04-14 | High | Current resolver chain is explicit: exact match, versioned match, explicit `outputContract`, then stem inference. | Proposal can still under-port contract binding while claiming parity. | Contract-resolution baseline. |
| MAP-02 | `control-plane/crates/workflow/src/compiler.rs` | Rust workflow | current compiled-schema seam | 2026-04-14 | High | Current Rust compiler still resolves schemas only through normalized-name / stem lookup. | Proposal can widen `OutputSchema` without fixing real binding behavior. | Compiler seam. |
| MAP-03 | `examples/agents/agents.yaml` | Current catalog | real explicit-contract users | 2026-04-14 | High | Current proposal-review agents output `proposal_review_po` / `_ux` / `_ui` / `_architect` but declare `output_contract: proposal_review_v1`, whose normalized artifact name is `proposal_review_normalized`. | Missing explicit-contract parity would skip validation on a current mandatory slice. | Real repo example. |
| MAP-04 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` and `Chainworks Forge/Engine/OutputContractSchemaV2.swift` | Stable reference | companion-mode semantics | 2026-04-14 | High | Stable semantics require `structured_with_human_companion` to persist both artifacts. | Proposal can still weaken a declared contract mode. | Mode-parity anchor. |
| MAP-05 | `control-plane/crates/graphql-server/src/types/stage.rs` | Rust GraphQL | stage summary reader | 2026-04-14 | High | Current stage GraphQL exposes summary booleans only. | Proposal can stop at `has_validation_failure` and still miss actual record details. | Stage reader baseline. |
| MAP-06 | `control-plane/crates/graphql-server/src/types/artifact.rs` and `control-plane/crates/mcp-server/src/server.rs` | Rust GraphQL + MCP | artifact/report readers | 2026-04-14 | High | Current artifact/report readers expose metadata and file paths, not decoded validation-failure payloads. | Proposal can overclaim “same durable truth” while delivering metadata only. | Report/resource baseline. |
| MAP-07 | `Chainworks Forge/Engine/RunReportBuilder.swift` and `Chainworks Forge/Engine/RecoveryCoordinator.swift` | Swift runtime | stable decoded-failure consumers | 2026-04-14 | High | Current stable readers decode `ValidationFailureRecord` from canonical stage/agent storage and use its contents directly. | Proposal needs a concrete daemon-side payload path if it claims parity. | Consumer baseline. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | contract-binding parity | Baseline + current repo | 2026-04-14 | High | Stable resolver parity already depends on explicit `output_contract` and non-normalized output names. | Proposal currently widens schema fields without specifying that binding behavior. | Primary blocker. |
| INT-02 | structured-output mode parity | Baseline + proposal | 2026-04-14 | High | Stable reference says `structured_with_human_companion` must persist both artifacts. | Proposal still treats the companion as informational. | Semantic blocker. |
| INT-03 | current northbound failure-truth delivery | Baseline + current repo | 2026-04-14 | High | Current daemon readers expose presence/metadata, while stable consumers need decoded record content. | Proposal still does not choose a payload surface. | Reader-boundary blocker. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Envelope extraction from ACP response text | Specified | DOC-01 | Rust ACP / executor seam | Direction is concrete and grounded. |
| Canonical metadata/path binding before persistence | Specified | DOC-01 | executor + compiled plan | Old artifact-binding blocker is closed at proposal-text level. |
| `ValidationFailureRecord` continuity fields | Specified | DOC-01 | failure-evidence record | Old receipt/transcript/raw-identity blocker is closed. |
| Projection-backed `has_validation_failure` ownership | Specified | DOC-01, MAP-05 | `stage_summaries` + GraphQL/MCP stage readers | Old duplicate-owner blocker is closed. |
| Explicit `output_contract` / full resolver-chain parity | Partial | DOC-01, MAP-01, MAP-02, MAP-03, INT-01 | compiler + catalog | Still not specified in the draft. |
| `structured_with_human_companion` parity | Contradicted by repo | DOC-01, DOC-03, MAP-04, INT-02 | stable reference mode semantics | Draft still weakens the stable rule. |
| Full decoded validation-failure payload on current northbound readers | Partial | DOC-01, DOC-04, MAP-05, MAP-06, MAP-07, INT-03 | GraphQL + MCP report/resource surfaces | Draft still stops at presence/metadata. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | stable contract slice | explicit contract resolution | current Swift resolver and current catalog examples prove non-normalized explicit-contract cases exist | proposal acceptance should prove `proposal_review_po`-style outputs still bind and validate | 2026-04-14 | High | Current acceptance does not prove this. |
| TEST-02 | stable contract slice | companion-mode semantics | stable reference says both artifacts must persist | proposal acceptance should prove companion persistence or explicitly defer it | 2026-04-14 | High | Current acceptance still treats companion absence as non-blocking. |
| TEST-03 | current daemon boundary | northbound failure-truth delivery | current readers only show booleans / metadata | proposal acceptance should prove a concrete decoded-payload path through current GraphQL / MCP readers | 2026-04-14 | High | Current acceptance overclaims northbound parity. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | compiler contract binding | current draft implies widening `OutputSchema` is enough | current resolver parity depends on explicit `output_contract` and current catalog examples already use it | 2026-04-14 | High | One core parity gap remains. |
| REAL-02 | companion-mode semantics | current draft says companion absence still passes | stable reference says both artifacts must persist for `structured_with_human_companion` | 2026-04-14 | High | One semantic contradiction remains. |
| REAL-03 | northbound failure truth | current draft says GraphQL / MCP readers consume the same durable truth | current daemon readers still expose only booleans and artifact metadata | 2026-04-14 | High | One reader-chain gap remains. |
| REAL-04 | old continuity blocker | old review basis said the draft omitted receipt/transcript/raw-normalized identity | current draft now explicitly carries all of those fields | 2026-04-14 | High | Earlier blocker is stale. |
| REAL-05 | old ownership blocker | old review basis said the draft reopened `has_validation_failure` on canonical stage rows and omitted MCP resources | current draft now keeps `has_validation_failure` on `stage_summaries` and names current MCP resources | 2026-04-14 | High | Earlier blocker is stale. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, MAP-02 | Envelope parsing and post-generation validation remain real daemon gaps. |
| Scope boundaries | Partial | DOC-01, DOC-04, INT-03 | Scope is mostly clear, but northbound payload ownership is still incomplete. |
| Reusable baseline coverage | Partial | BASE-02, BASE-03, REAL-01, REAL-03 | Draft aligns much better than before, but not fully. |
| Data / execution contract | Partial | MAP-01, MAP-02, MAP-03, INT-01 | Contract-binding parity is still under-specified. |
| Failure / recovery semantics | Partial | DOC-03, MAP-07, INT-03 | Durable evidence exists, but current northbound consumers still lack a concrete payload path. |
| Testing strategy | Partial | TEST-01, TEST-02, TEST-03 | Acceptance still misses resolver-parity proof and full northbound payload proof. |
| Dependencies / integration points | Partial | DOC-04, MAP-05, MAP-06 | Reader surfaces are named, but the full decoded record path still is not. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P046` intends real parity with the stable contract slice, not a narrower Rust-only approximation.
- ASSUMP-02: Current proposal-review outputs remain a mandatory adopter for the Rust port, just as they are in the stable reference slice.
- QUESTION-01: Which current northbound reader should carry the decoded `ValidationFailureRecord` payload: `report://{run_id}`, `reports.get`, GraphQL artifact surfaces, or a linked current-reader object?
- BLOCKER-01: the draft still does not specify full compiler resolver parity for explicit `output_contract` users.
- BLOCKER-02: the draft still weakens `structured_with_human_companion`.
- BLOCKER-03: the draft still does not choose a current-boundary payload surface for the decoded failure record.

## L. Research Triggers / External Questions
No external research trigger was required. Local proposal/docs/code/baseline evidence is sufficient for a proposal-readiness verdict.
