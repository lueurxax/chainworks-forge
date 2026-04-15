# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/acp-runtime-transport.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/proposals/046-structured-output-envelope-and-contract-validation.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/acp-runtime-transport.md`
  - `docs/reference/rust-control-plane.md`
- Baseline refreshed:
  - targeted reread of stable failed-stage evidence and recovery packet owners
  - targeted reread of stable MCP registry loading and ACP session materialization
  - targeted code refresh for current Rust start-run, ACP request/result, stage/agent rows, report resource, and northbound read seams
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift`
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `control-plane/crates/acp/src/lib.rs`
  - `control-plane/crates/acp/src/transport.rs`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/domain/src/run.rs`
  - `control-plane/crates/domain/src/stage.rs`
  - `control-plane/crates/domain/src/validation.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/workflow/src/catalog.rs`
  - `control-plane/crates/workflow/src/compiler.rs`
  - `control-plane/crates/workflow/src/plan.rs`
  - `control-plane/crates/db/src/repos/runs.rs`
  - `control-plane/crates/db/src/repos/stages.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/graphql-server/src/types/run.rs`
  - `control-plane/crates/graphql-server/src/types/stage.rs`
- Current repo contradictions found:
  - the old review basis is stale: the current draft already fixes the prior undeclared `P046` dependency gap and now explicitly locks missing/disabled backend MCP servers as fail-closed against the canonical ACP registry contract
  - three new live proposal-first blockers remain:
    - the proposal promises the full stable failed-stage evidence packet without specifying the Rust owner fields needed to build it
    - the MCP resolution contract still omits runtime-scoped inputs and weakens current live-registry semantics
    - the delivery preflight result schema no longer matches the stable frozen JSON contract

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `The stale red basis is gone, but the current draft still overclaims packet parity, under-specifies runtime-scoped MCP resolution, and drifts the delivery preflight JSON contract.`
- Top residual implementation risks:
  1. The promised `FailedStageEvidencePacket` cannot be built with current Rust durable owners as specified in the draft.
  2. MCP resolution still lacks the runtime namespace / provider-binding inputs and live registry reload semantics that the stable Swift owner chain uses.
  3. The delivery preflight payload shape diverges from the stable `DeliveryPreflightService.PreflightResult` contract that current UI and frozen run evidence already assume.

## 2. Proposal Scope and Completeness
- In scope:
  - stage-owned failed-stage evidence in Rust
  - run-creation delivery preflight for frozen delivery config
  - ACP `mcpServers` realization from `backend_profile.mcp`
- Out of scope:
  - shell-owned evidence-pack export
  - release-time readiness heuristics
  - implementation audit or gate execution
- Most important baseline refreshes performed:
  - stable failed-stage packet owner chain
  - stable delivery-preflight persisted shape
  - stable runtime-scoped MCP resolution and registry loading path
  - current Rust ACP / DB / GraphQL / MCP read surfaces
- Most important contradictions with current repo:
  - the earlier dependency / fail-policy / registry-path blockers are now closed in the draft
  - the failed-stage packet still lacks its Rust source-field owner map
  - the MCP section still leaves out runtime-scoped inputs and live registry timing
  - the delivery preflight section still changes the stable persisted result schema

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
| Architecture | Red | High | Complete | 1 | 1 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` finding.

### 5.2 UX Findings
- No live UX `proposal-text` finding.

### 5.3 Architecture Findings

#### ARCH-001 - The proposal still overclaims full failed-stage packet parity without naming the Rust owner fields needed to build it
- Severity: `Critical`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-03`, `DOC-06`, `MAP-01`, `MAP-02`, `MAP-03`, `INT-01`
- Proposal refs:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:68`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:100`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:102`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:315`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:327`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:345`
- Current repo refs:
  - `control-plane/crates/acp/src/lib.rs:13`
  - `control-plane/crates/domain/src/agent.rs:40`
  - `control-plane/crates/domain/src/stage.rs:83`
  - `control-plane/crates/domain/src/validation.rs:50`
  - `control-plane/crates/engine/src/executor.rs:193`
  - `control-plane/crates/engine/src/executor.rs:259`
  - `control-plane/crates/engine/src/executor.rs:349`
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift:12`
- Why it matters:
  - The draft now promises the **full stable** `FailedStageEvidencePacket`, including `supervision_classification`, `canonical_outcome`, `transport_error_kind`, `output_presence`, `output_envelopes`, and `recovery_snapshot`. But the current Rust substrate does not persist most of those source fields. `acp::ExecutionResult` only returns status, artifact paths, and cost; `domain::AgentExecution` only stores provider/model/status/timestamps; `domain::StageExecution` does not carry `recovery_snapshot`; and the file list adds MCP provenance to `domain/src/agent.rs` but no failed-stage packet source fields. That means the orchestrator cannot actually build the claimed parity packet at failure settlement from the owners named in the proposal.
- Required fix:
  - Either narrow the packet contract explicitly, or add the missing owner map and persistence seam for the full parity packet.
  - At minimum, the proposal needs to name where Rust will persist or reconstruct:
    - `canonical_outcome`
    - `transport_error_kind`
    - `output_presence`
    - `output_envelopes`
    - `supervision_classification`
    - `recovery_snapshot`
  - If those owners live across `acp::ExecutionResult`, `domain::AgentExecution`, `domain::StageExecution`, validation rows, and recovery logic, the file inventory and acceptance criteria must say so directly.

#### ARCH-002 - MCP resolution still omits the runtime-scoped inputs and reload timing that the stable owner chain depends on
- Severity: `High`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `DOC-04`, `MAP-04`, `MAP-05`, `INT-02`
- Proposal refs:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:214`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:236`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:242`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:263`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:288`
- Current repo refs:
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:247`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:152`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift:162`
  - `Chainworks Forge/Engine/PreflightService.swift:662`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift:37`
- Why it matters:
  - The proposal correctly moved executable MCP definitions to the canonical machine-local ACP registry and correctly made missing/disabled requested servers fail-closed. But the current design still leaves out two runtime-critical details that the stable Swift owner chain already uses:
    - `resolve_mcp_servers(requested_ids, registry)` does not take the selected runtime namespace / provider binding, even though the proposal's own rules depend on it for cases like `type == "platform"` being valid only for Codex.
    - the proposal says the registry is loaded at daemon startup, while the stable system reads a fresh snapshot during preflight and again during ACP realization. Startup-only loading weakens current operator semantics because MCP registry edits would not be seen until daemon restart.
- Required fix:
  - Thread the runtime-scoped inputs into the MCP resolution and realization contract. The design needs either `ResolvedProviderBinding`, runtime namespace, or an equivalent selected-runtime input at the resolver boundary.
  - Make registry loading semantics explicit and parity-safe: either read fresh snapshots for preflight/session creation like the current Swift owners, or explicitly document the intended runtime-staleness tradeoff and update stable-reference claims.
  - Keep the resolver contract clear about which step performs:
    - requested-intent validation
    - runtime-ID mapping
    - ACP `mcpServers` realization
    - post-session actual/denied settlement

#### ARCH-003 - Delivery preflight no longer matches the stable frozen JSON contract it claims to port
- Severity: `Medium`
- Confidence: `High`
- Evidence Completeness: `Complete`
- Evidence IDs: `DOC-01`, `MAP-06`, `MAP-07`, `INT-03`
- Proposal refs:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:118`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:124`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:135`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md:164`
- Current repo refs:
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift:9`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift:34`
  - `Chainworks Forge/Views/IdeaListView.swift:1686`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:59`
- Why it matters:
  - The proposal describes `DeliveryPreflightResult.PreflightCheck` as `{ name, passed, detail }`, but the stable persisted shape is `{ id, label, passed, detail }`. Current UI and blocking-reason surfaces use `failedChecks.map(\\.id)` and render `check.label`, while evidence-pack export writes `deliveryPreflightJSON` as-is. That means the proposal is no longer a strict port of the stable preflight payload contract.
- Required fix:
  - Keep the current frozen result shape unless the proposal intentionally includes a cross-reader migration.
  - If the Rust daemon wants a different internal representation, that is fine, but the persisted JSON contract should stay aligned with Swift `DeliveryPreflightService.PreflightResult` or the proposal must explicitly update every downstream reader.

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  - the proposal claims full failed-stage packet parity while naming only a subset of the owner fields required to build it
  - decision needed: full parity with new source-field owners, or intentionally narrower packet
- Conflict:
  - the MCP section claims Swift-parity resolution, but its resolver signature and registry load timing are weaker than the current runtime-scoped owner chain
  - decision needed: runtime-scoped parity now, or explicit documented behavior change
- Conflict:
  - the delivery preflight section claims a service port but changes the persisted check schema
  - decision needed: preserve the current JSON contract, or promote a contract migration slice

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Add the missing Rust owner map for the full failed-stage packet, or narrow the packet contract explicitly | Architecture | proposal author | Before next review | current ACP / executor / recovery seams | every promised packet field has a named durable source | `ARCH-001` |
| P1 | Make MCP resolution explicitly runtime-scoped and align registry load timing with the stable owner chain | Architecture | proposal author | Before next review | current Swift MCP owner flow | resolver contract includes selected runtime inputs and load timing is unambiguous | `ARCH-002` |
| P2 | Restore the stable delivery preflight JSON payload shape or document a reader migration | Architecture | proposal author | Before next review | current preflight UI/evidence readers | persisted delivery-preflight payload is no longer ambiguous | `ARCH-003` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Failed-stage evidence parity | whether every promised packet field has a durable Rust owner | file inventory names all required owners | no hidden reconstruction from ad hoc scans | next proposal review | hold if packet still overclaims unavailable fields |
| MCP resolution parity | whether requested MCP intent is validated against the selected runtime and live registry snapshot | resolver signature includes runtime-scoped input | no startup-only stale registry assumption unless explicitly accepted | next proposal review | hold if runtime namespace / reload timing stays implicit |
| Delivery preflight parity | whether frozen preflight payload stays reader-compatible | persisted JSON matches stable field names | no silent contract drift in evidence/export path | next proposal review | hold if payload shape remains divergent |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient.

### Open Questions
- QUESTION-01: Does the author want true field-for-field failed-stage packet parity, or a narrower Rust V1 packet with an explicit reader-contract delta?
- QUESTION-02: Should daemon MCP registry reads remain live per preflight/session like Swift, or is a startup-cached registry an intentional operational tradeoff?
- QUESTION-03: Is there any consumer that actually requires a renamed delivery-preflight check schema, or was `{ name }` just placeholder wording that should revert to `{ id, label }`?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
