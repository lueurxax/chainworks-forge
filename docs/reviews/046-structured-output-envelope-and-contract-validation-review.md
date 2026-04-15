# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Reviewed on: `2026-04-15`
- Reviewed tree: working tree rooted at commit `ddc5c0d52aff` with local modifications present in the Rust control-plane crates
- Proposal / docs reviewed:
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/rust-control-plane.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/rust-control-plane.md`
- Baseline refreshed:
  - targeted reread of the stable output-contract / failure-evidence reference
  - targeted reread of the current stage-owned retry / recovery reference
  - targeted code refresh for current Rust catalog parsing, projection schema, GraphQL artifact and stage types, and MCP report/resource readers
  - targeted working-tree diff review for current draft changes in `workflow/src/catalog.rs`, `workflow/src/plan.rs`, `mcp-server/src/tools/reports.rs`, and `mcp-server/src/server.rs`
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `control-plane/crates/workflow/src/catalog.rs`
  - `control-plane/crates/workflow/src/compiler.rs`
  - `control-plane/crates/workflow/src/plan.rs`
  - `control-plane/crates/db/migrations/002_projections.sql`
  - `control-plane/crates/db/src/repos/projections.rs`
  - `control-plane/crates/graphql-server/src/types/artifact.rs`
  - `control-plane/crates/graphql-server/src/types/stage.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `control-plane/crates/mcp-server/src/tools/stages.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
- Current repo contradictions found:
  - the previous attempt-lineage blocker is now closed in the draft: `validation_failure_records` is keyed with `stage_execution_id` and `agent_execution_id`, and `stage_summaries.has_validation_failure` is derived by exact `stage_execution_id`
  - the previous northbound-owner blocker is now closed in the draft: `validation_failure_records.record_json` is named as the authoritative typed payload source for `GqlArtifact`, `reports.get`, and `report://{run_id}`
  - the previous `human_format` parser-owner blocker is now closed in the draft: `workflow/src/catalog.rs` is explicitly named in both the design text and affected-files list
  - the draft also preserves the earlier explicit `output_contract` and `structured_with_human_companion` fixes
  - fresh working-tree changes in `workflow/src/catalog.rs` and `workflow/src/plan.rs` add skill/worktree metadata, but they still do not carry `human_format` through the Rust compiled `OutputSchema`, so the proposal's schema-parity work remains correctly scoped
  - fresh working-tree changes in `mcp-server/src/tools/reports.rs` and `mcp-server/src/server.rs` widen release-report and canonical-run reads, but they still do not surface decoded `ValidationFailureRecord` payloads or a stage-level `has_validation_failure` bit, so the proposal's northbound work remains necessary
  - no new proposal-blocking contradiction was found against the current repo topology
- Remaining blockers:
  - none

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `The current draft now resolves the prior owner-chain gaps and is implementable against the current repo seams`
- Residual implementation watchpoints:
  1. Keep decoded validation-failure payload delivery anchored to `validation_failure_records.record_json`; do not regress to metadata-only or boolean-derived reconstruction during implementation.
  2. Prove same-run retry isolation with fixtures that create a failed first attempt and a successful later attempt under the same logical stage.
  3. Prove full catalog-to-plan propagation for `human_format`, not just `validation_mode` and `machine_format`.

## 2. Proposal Scope and Completeness
- In scope:
  - envelope extraction from ACP responses
  - post-generation output validation using the stable owner chain
  - durable validation-failure persistence
  - canonical metadata/path binding before persistence
  - stage/artifact surfacing of validation-failure truth
- Out of scope:
  - broader thin-client cutover
  - unrelated release / approval proposals
  - runtime gate execution
- Most important baseline refreshes performed:
  - stable same-run retry / stage-owned failure-truth rules
  - current Rust projection and report/resource reader topology
  - current Rust catalog contract parser shape
- Most important confirmations against current repo:
  - the proposal now restores full contract-binding parity, including explicit `output_contract`
  - the proposal now keeps validation-failure truth attempt-aware instead of flattening by logical stage
  - the proposal now assigns one concrete typed payload source to the current GraphQL / MCP readers
  - the proposal now names the missing Rust parser owner for `human_format`
  - fresh working-tree MCP and workflow-parser changes do not invalidate the proposal; they only confirm that the proposal is still aimed at a live, unsolved seam

## 3. Proposal Readiness Verdict
- `Readiness = Green`
- `Confidence = High`
- `Evidence Completeness = Complete`

This is **not** an Evidence Gap Review. Local proposal/docs/code/baseline evidence is sufficient for a proposal-first verdict.

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding.

### 5.2 UX Findings
- No live UX `proposal-text` finding.

### 5.3 Architecture Findings
- No live architecture `proposal-text` finding.

## 6. Cross-Discipline Conflicts and Decisions
- Locked decision:
  - attempt-aware validation-failure truth remains stage-execution-owned, not logical-stage-owned
- Locked decision:
  - typed northbound validation-failure payloads come from `validation_failure_records.record_json`
- Locked decision:
  - full schema parity includes `human_format` at the catalog parser boundary, not as a compiler-only default

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|---|
| P2 | Implement the proposal exactly as written and keep the read path tied to `record_json` | Architecture | implementation owner | Implementation | current GraphQL / MCP reader seams | decoded validation-failure payloads reach `GqlArtifact`, `reports.get`, and `report://{run_id}` without a second truth source | proposal text + evidence pack |
| P2 | Add retry-isolation coverage for failed-first-attempt then successful-retry scenarios | Architecture | implementation owner | Implementation | attempt-aware DB and projection updates | later attempts in the same run do not inherit stale validation-failure flags | proposal text + evidence pack |
| P2 | Add catalog-to-plan coverage for `human_format` propagation | Architecture | implementation owner | Implementation | catalog/compiler/plan changes | `human_format` survives YAML parse through compiled `OutputSchema` | proposal text + evidence pack |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Hold Criteria |
|---|---|---|---|---|---|
| Retry-lineage safety | whether validation-failure truth stays attached to the correct attempt | DB row includes `stage_execution_id` and `agent_execution_id`; projection derives by exact execution ID | no cross-attempt smearing on same-run retries | implementation audit | hold if later retries can still inherit stale validation-failure state |
| Northbound decoded payload delivery | whether current GraphQL / MCP readers surface the actual failure record | proposal-fixed typed payload source stays `record_json` end-to-end | no metadata-only overclaim | implementation audit | hold if any reader reconstructs payload from booleans or omits typed data |
| Contract-schema completeness | whether `human_format` is actually part of Rust catalog ingestion | parser, compiler, and plan all carry the field | no partial schema parity claim | implementation audit | hold if `human_format` is still dropped during YAML parse or compile |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: No blocking proposal question remains for the next review round. Any remaining loader or resolver shape choices are implementation details, not proposal-readiness gaps.

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
