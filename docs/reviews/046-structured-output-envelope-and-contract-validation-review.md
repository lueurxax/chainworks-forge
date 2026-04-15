# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/rust-control-plane.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/rust-control-plane.md`
- Baseline refreshed:
  - targeted reread of the stable output-contract / failure-evidence reference
  - targeted code refresh for current Swift `OutputContractResolverV2`, `ValidationFailureRecord`, `RunReportBuilder`, and `RecoveryCoordinator`
  - targeted code refresh for current Rust workflow compiler, GraphQL artifact/stage types, and MCP report/resource surfaces
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `control-plane/crates/workflow/src/catalog.rs`
  - `control-plane/crates/workflow/src/compiler.rs`
  - `control-plane/crates/workflow/src/plan.rs`
  - `control-plane/crates/graphql-server/src/types/stage.rs`
  - `control-plane/crates/graphql-server/src/types/artifact.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `examples/agents/agents.yaml`
- Current repo contradictions found:
  - the old `ValidationFailureRecord` omission basis is stale: the draft now explicitly preserves `receipt_exists`, `transcript_exists`, and raw/normalized artifact identity
  - the old duplicate `has_validation_failure` basis is stale: the draft now consistently keeps that truth on `stage_summaries`
  - the old MCP-resource omission basis is stale: the draft now explicitly names `chainworks://runs/{run_id}/stages` and `report://{run_id}`
  - three live proposal-first gaps remain:
    - the compiler binding path still does not specify the full current `OutputContractResolverV2` match chain, especially the explicit `output_contract` cases used by current proposal-review agents
    - `structured_with_human_companion` still treats the companion artifact as informational/optional, which conflicts with the stable reference contract
    - the northbound section still stops at stage booleans plus artifact metadata and does not say how GraphQL / MCP readers obtain the actual `ValidationFailureRecord` payload they need
- Remaining blockers:
  - incomplete compiler-to-contract parity for current explicit `output_contract` users
  - incorrect parity semantics for `structured_with_human_companion`
  - incomplete full-record delivery path for current northbound readers

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Materially stronger than the previous review basis, but still not implementation-ready`
- Top residual implementation risks:
  1. The draft widens `OutputSchema`, but it still does not say how Rust compilation restores the full current resolver chain (`exact`, versioned, explicit `output_contract`, stem). That is already material for the current proposal-review agents.
  2. The draft’s `structured_with_human_companion` rules still allow a pass without the companion artifact, contradicting the stable contract reference that says both artifacts must persist.
  3. The draft now wires `has_validation_failure` and `report_kind`, but current northbound readers still are not given a concrete path to the decoded `ValidationFailureRecord` payload itself.

## 2. Proposal Scope and Completeness
- In scope:
  - ACP response envelope extraction in the Rust daemon
  - post-response output validation using the stable owner chain
  - durable validation-failure evidence
  - canonical metadata/path binding before persistence
  - stage/artifact surfacing of validation failure truth
- Out of scope:
  - broader thin-client cutover
  - unrelated approval/release proposals
  - runtime gate or build verification
- Most important baseline refreshes performed:
  - stable `OutputContractResolverV2` contract-resolution behavior
  - stable structured-output mode semantics from the reference docs
  - current daemon GraphQL / MCP report and artifact reader boundary
- Most important contradictions with current repo:
  - current Swift resolver supports explicit `output_contract` binding for outputs whose names do not match the normalized artifact name, but the draft still talks only in terms of widening `CompiledTask.output_schemas`
  - current stable reference says `structured_with_human_companion` must persist both artifacts, while the draft still makes the companion non-blocking
  - current GraphQL / MCP readers expose only stage summary booleans and artifact metadata, while stable report/recovery readers consume the decoded `ValidationFailureRecord`

## 3. Proposal Readiness Verdict
- `Readiness = Red`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is **not** an Evidence Gap Review. Local proposal/docs/code/baseline evidence is sufficient for a proposal-first verdict.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Red | High | Complete | 0 | 2 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding. The draft is not blocked on shell layout or navigation ownership.

### 5.2 UX Findings
- No live UX `proposal-text` finding. Operator intent remains clear: named outputs, canonical validation, and durable failure evidence.

### 5.3 Architecture Findings

#### ARCH-001 - Compiler contract binding still does not restore the full current resolver chain
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-03`, `MAP-01`, `MAP-02`, `MAP-03`, `INT-01`, `REAL-01`
- Proposal refs:
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:86`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:120`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:327`
- Current repo refs:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift:40`
  - `control-plane/crates/workflow/src/compiler.rs:86`
  - `control-plane/crates/workflow/src/compiler.rs:374`
  - `examples/agents/agents.yaml:107`
  - `examples/agents/agents.yaml:1073`
- Why it matters:
  - The draft now widens `OutputSchema`, but it still never says how Rust compilation regains the full current contract-binding behavior that Swift already treats as canonical: exact contract-name match, versioned match, agent-level explicit `output_contract`, and stem inference. That omission is already live in the repo. Current proposal-review agents emit outputs like `proposal_review_po`, `proposal_review_ux`, `proposal_review_ui`, and `proposal_review_architect`, while their declared contract is `proposal_review_v1` and the contract’s normalized artifact name is `proposal_review_normalized`. If Rust only keeps its current normalized-name/stem lookup and merely copies more fields into `OutputSchema`, these outputs still will not bind to a contract and will bypass the validation path the proposal says it is porting.
- Required fix:
  - Specify that the Rust compiler / plan binding must preserve the full current `OutputContractResolverV2` resolution chain when populating `CompiledTask.output_schemas`, including explicit `output_contract`.
  - Make the acceptance bar prove that current proposal-review outputs still bind to `proposal_review_v1` even though their output names do not match the normalized artifact name.

#### ARCH-002 - `structured_with_human_companion` still weakens the stable contract semantics
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-03`, `MAP-04`, `INT-02`, `REAL-02`
- Proposal refs:
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:171`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:177`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:348`
- Current repo refs:
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md:64`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift:12`
- Why it matters:
  - The draft now carries `human_format`, `raw_artifact_name`, and `normalized_artifact_name`, but its concrete rule still says the machine artifact passes regardless of whether the companion exists. That is weaker than the stable reference contract for this slice, which explicitly says `structured_with_human_companion` must persist both the machine-valid artifact and the human companion artifact. As written, Rust can claim parity while allowing a contract mode that the current reference model says is incomplete.
- Required fix:
  - Rebind the mode semantics to the stable reference: a successful `structured_with_human_companion` result must persist both artifacts.
  - If the author intends to defer companion persistence, the proposal must say so explicitly and narrow its parity claim instead of describing the current behavior as equivalent.

#### ARCH-003 - Northbound readers still are not given a concrete full-record delivery path
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-04`, `MAP-05`, `MAP-06`, `MAP-07`, `INT-03`, `REAL-03`
- Proposal refs:
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:271`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:299`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md:347`
- Current repo refs:
  - `control-plane/crates/graphql-server/src/types/stage.rs:6`
  - `control-plane/crates/graphql-server/src/types/artifact.rs:6`
  - `control-plane/crates/mcp-server/src/server.rs:314`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:656`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:624`
- Why it matters:
  - The draft now correctly routes stage status through `stage_summaries` and validation-failure artifacts through the existing report lane, but that still only exposes a boolean plus artifact metadata. Current stable report and recovery readers do not work from presence alone; they decode the actual `ValidationFailureRecord` payload and use fields like `failureSummary`, `missingFields`, and `recoveryRecommendation`. The current daemon’s `GqlStageExecution`, `GqlArtifact`, `reports.get`, and `report://{run_id}` resource do not expose that payload today, and the draft still does not say which of those readers gains it. As written, implementation can satisfy the current text while leaving northbound operator/report consumers blind to the canonical failure details.
- Required fix:
  - Add a concrete owner path for the decoded `ValidationFailureRecord` payload on the current northbound boundary.
  - State whether that payload is surfaced through `report://{run_id}`, `reports.get`, GraphQL artifact types, or a linked report object, and update the file list / acceptance criteria accordingly.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  - The draft claims OutputContractResolverV2 parity, but its compiler section still does not say how Rust recovers explicit `output_contract` and other non-normalized binding cases.
  - Decision needed: either port the full resolver chain or narrow the parity claim.
- Conflict:
  - The draft says `structured_with_human_companion` matches the stable owner model, but its rule text still allows success without the companion artifact.
  - Decision needed: keep the stable “both artifacts persist” rule or explicitly defer it.
- Conflict:
  - The draft says GraphQL and MCP readers consume the same durable truth, but it only wires presence metadata.
  - Decision needed: choose one concrete northbound payload path for the decoded failure record.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Re-specify compiler contract binding so `CompiledTask.output_schemas` preserves the full current `OutputContractResolverV2` match chain | Architecture | proposal author | Before next review | current catalog + compiler behavior | proposal proves explicit `output_contract` users still bind to contracts | `ARCH-001` |
| P1 | Add a concrete northbound path for the decoded `ValidationFailureRecord` payload | Architecture | proposal author | Before next review | current GraphQL / MCP reader boundary | proposal no longer stops at stage booleans and artifact metadata | `ARCH-003` |
| P2 | Tighten `structured_with_human_companion` to the stable “persist both artifacts” rule or explicitly defer it | Architecture | proposal author | Before next review | stable reference semantics | proposal no longer weakens that validation mode | `ARCH-002` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Contract-binding parity | whether current explicit `output_contract` users still resolve to contracts | acceptance includes a proposal-review output example that binds to `proposal_review_v1` | no normalized-name-only regression | next proposal review | hold if the compiler section still relies on normalized-name lookup alone |
| Companion-mode parity | whether `structured_with_human_companion` matches the stable reference | proposal requires companion persistence for success | no informational-only companion wording | next proposal review | hold if the draft still passes without the companion artifact |
| Northbound failure-truth delivery | whether current GraphQL / MCP readers can read the actual failure record | proposal names one concrete payload surface for decoded `ValidationFailureRecord` | no metadata-only overclaim | next proposal review | hold if acceptance still treats stage booleans + `report_kind` metadata as sufficient |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: Should the decoded `ValidationFailureRecord` payload ride in `report://{run_id}`, `reports.get`, a GraphQL artifact field, or a new linked report object on the existing readers? The proposal should choose one concrete current-boundary owner.

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
