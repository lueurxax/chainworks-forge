# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/acp-runtime-transport.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/per-agent-mcp-policy-and-runtime-validation.md`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/output-contracts-failure-evidence-and-recovery.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/acp-runtime-transport.md`
- Baseline reused:
  - repo-level intake and current proposal/readiness framing from `.review-baselines/current-system-baseline.md`
- Baseline refreshed:
  - failure-evidence ownership and artifact namespace
  - loop iteration and retry semantics
  - stable delivery-preflight frozen JSON contract
  - runtime-scoped MCP resolution owner chain
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context: none
- Targeted context refresh performed:
  - Rust workflow compiler, plan, executor, orchestrator, ACP transport, and DB/domain seams
  - Swift stable owner/read surfaces for delivery preflight, failed-stage evidence, and MCP policy resolution
- External research used: `None`
- Code areas inspected:
  - `Chainworks Forge/Engine/DeliveryPreflightService.swift`
  - `Chainworks Forge/Views/DeliveryPreflightReportView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Engine/MCPPolicyRuntime.swift`
  - `Chainworks Forge/Engine/RuntimeSessionBridge.swift`
  - `Chainworks Forge/Engine/RunPlan.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
  - `control-plane/crates/workflow/src/catalog.rs`
  - `control-plane/crates/workflow/src/plan.rs`
  - `control-plane/crates/workflow/src/compiler.rs`
  - `control-plane/crates/engine/src/orchestrator.rs`
  - `control-plane/crates/engine/src/command_handler.rs`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/acp/src/lib.rs`
  - `control-plane/crates/acp/src/transport.rs`
  - `control-plane/crates/domain/src/stage.rs`
  - `control-plane/crates/domain/src/agent.rs`
  - `control-plane/crates/db/src/repos/runs.rs`
  - `control-plane/crates/db/src/repos/stages.rs`
  - `control-plane/crates/db/src/repos/agent_executions.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
- Current repo contradictions found:
  - the previous P048 review artifacts are stale; the current draft already closes the old undeclared `P046` dependency, delivery-preflight payload-shape, and canonical MCP registry / fail-closed blockers
  - `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` is partially stale against current code: live code treats `backend_profile.mcp` as canonical MCP ownership while the doc still describes legacy `mcp_profile` ownership
- Runtime evidence used: `None`
- Provenance of key evidence:
  - proposal text for claimed contract changes
  - current Swift stable owners/readers for parity reference
  - current Rust execution/storage seams for implementability
  - stable reference docs for artifact and recovery rules
- Remaining assumptions:
  - `McpResolutionReport.profile_id` is intended to mean backend profile ID, matching Swift `MCPPolicyResolutionReport.profileID`
  - `FailedStageEvidencePacket.failed_agent_title` is still intended as a useful operator-facing field, not dead schema baggage
- Remaining blockers:
  - failed-stage evidence artifact namespace / canonical path semantics
  - missing explicit Rust owner for MCP `profile_id`
  - missing explicit Rust owner or explicit V1 deferral for `failed_agent_title`

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top risks:
  1. The proposed failed-stage evidence artifact path can collide across loop iterations and mixes export-pack naming with the repo's canonical artifact namespace model.
  2. `McpResolutionReport.profile_id` has no named Rust owner in the proposed compiler/runtime path.
  3. `failed_agent_title` remains in the packet contract, but the current Rust durable owner chain does not carry title today.
- Top opportunities:
  1. The old blocker set is genuinely closed: explicit `P046` dependency, stable delivery-preflight payload shape, and canonical MCP registry / fail-closed semantics are now present in the draft.
  2. Failed-stage evidence is now scoped as Rust V1 with explicit nullable parity-deferred fields instead of overclaiming immediate full parity.
  3. MCP resolution now targets the same machine-local registry contract already used by the Swift implementation.

## 2. Proposal Scope and Completeness
- In scope:
  - stage-owned failed-stage evidence in Rust
  - run-creation delivery preflight for frozen delivery config
  - ACP `mcpServers` realization from `backend_profile.mcp`
- Out of scope:
  - shell-owned evidence-pack export
  - release-time readiness heuristics
  - implementation audit / proof execution
- Deferred intentionally:
  - start-time `PreflightService` MCP warning parity
  - full V2 failed-stage packet parity for fields the proposal already marks nullable
- Most important baseline refreshes performed:
  - loop iteration and retry namespace semantics
  - stable delivery-preflight frozen payload contract
  - stable MCP `profileID` ownership chain
  - current Rust owner gaps for title / backend-profile propagation
- Most important contradictions with current repo:
  - stale review artifacts still describe blockers that the current proposal text has already fixed
  - the MCP ownership reference doc is behind current code and should not be treated as authoritative over the working tree
- Most important missing or partial states:
  - disjoint failed-stage evidence artifact identity across iteration and retry
  - explicit owner for backend profile ID in MCP report persistence
  - explicit owner or explicit nullability rule for failed agent title

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| Architecture | Amber | High | Complete | 0 | 1 | 2 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI `proposal-text` findings.

### 5.2 UX Findings
- No live UX `proposal-text` findings.

### 5.3 Architecture Findings
- Finding ID: `ARCH-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-04`, `MAP-01`, `MAP-02`, `INT-01`, `REAL-03`
  Why it matters:
  The proposal says failed-stage evidence should be written to the canonical artifact path as `failure-evidence/evidence-{stage_id}-attempt{n}.json`. That path omits loop iteration or stage-execution identity. Current workflow execution reuses the same `stage_id` across loop iterations, creates a fresh iteration with `attempt_number = 1`, and only increments `attempt_number` on same-stage retry. The stable artifact model uses `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}` precisely to keep retries and loop iterations disjoint. Swift's flat `failure-evidence/evidence-<stageID>-attempt<n>.json` naming exists today only inside the export pack builder, not as canonical runtime artifact storage. Porting that flat export name into runtime artifact persistence risks overwrite and mixes two different namespace models.
  Recommended fix:
  Use a canonical identity that cannot collide, such as `stage_execution_id`, or include `{stage_id}.{iteration}` in the persisted artifact name/path. Better yet, persist the report artifact through the normal artifact row path and let export builders flatten later if they want a human bundle.
  Acceptance criteria:
  - the same logical stage failing at `iteration=1, attempt=1` and `iteration=2, attempt=1` produces two distinct failed-stage evidence artifacts
  - both artifacts remain discoverable via persisted artifact rows and `reports.get`
  - proposal text no longer labels a flat export-style path as the canonical artifact path
  Confidence: `High`

- Finding ID: `ARCH-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-03`, `MAP-04`, `MAP-07`, `INT-02`, `REAL-04`
  Why it matters:
  The proposal introduces `McpResolutionReport.profile_id`, but the proposed Rust changes only mention adding `requested_mcp_server_ids` and runtime binding to `ResolvedAgent`. Current Rust `ResolvedAgent` and `AgentBinding` do not carry backend profile ID, while stable Swift MCP policy resolution derives `profileID` from `ResolvedAgent.backendProfileID`. Without an explicit Rust owner, `profile_id` can only be reconstructed by re-reading catalog YAML at execution/report time, which would weaken the claimed persistence contract.
  Recommended fix:
  Add `backend_profile_id` to Rust `ResolvedAgent` and compiler output, then persist/report that value directly. If `profile_id` is intended to mean something else, rename it now and state the owner explicitly in the file inventory and acceptance criteria.
  Acceptance criteria:
  - `McpResolutionReport.profile_id` can be emitted from persisted execution truth without re-reading YAML
  - the proposal file inventory explicitly names the owner field added to Rust `ResolvedAgent` / execution persistence
  Confidence: `High`

- Finding ID: `ARCH-003`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-03`, `MAP-04`, `MAP-08`, `INT-01`, `REAL-05`
  Why it matters:
  `FailedStageEvidencePacket` still contains `failed_agent_title`, and the stable Swift packet populates it from `failedAgent.agentTitle`. Current Rust `ResolvedAgent`, `AgentExecution`, and `StageExecution` do not carry title. The proposal says the packet is built immediately when a stage settles, but it does not name any Rust owner for title and does not explicitly list this field in the V1 nullable/deferred set. That leaves the operator-facing packet schema more optimistic than the actual current owner chain.
  Recommended fix:
  Either add title to the Rust owner chain that feeds failed-stage evidence, or state explicitly that `failed_agent_title` is nullable in Rust V1 until a later parity slice adds the owner.
  Acceptance criteria:
  - the proposal names a durable Rust owner for failed agent title, or
  - the proposal explicitly marks `failed_agent_title` as optional / expected `null` in Rust V1 readers and acceptance text
  Confidence: `High`

## 6. Cross-Discipline Conflicts and Decisions
- Conflict:
  The proposal now correctly preserves stable delivery-preflight payload shape and canonical MCP registry semantics, but the remaining architecture gaps are namespace/ownership gaps rather than flow/UX gaps.
  Tradeoff:
  Pushing the proposal to `Green` now requires tightening identity and owner declarations, not reopening scope.
  Decision:
  Keep scope as-is and fix the remaining contract details in-text rather than splitting a follow-on proposal.
  Owner:
  proposal author

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Replace flat failed-stage evidence artifact naming with a collision-free canonical identity | Architecture | proposal author | Before next review | current loop/retry semantics | failed-stage evidence artifacts stay unique across iteration and retry | `ARCH-001` |
| P1 | Add explicit Rust owner for MCP `profile_id` or rename the field to match available truth | Architecture | proposal author | Before next review | compiler / resolved-agent changes | MCP resolution reports can be persisted without catalog re-read | `ARCH-002` |
| P1 | Add explicit owner or explicit V1 deferral for `failed_agent_title` | Architecture | proposal author | Before next review | failed-stage packet contract | packet readers know whether title is durable or nullable | `ARCH-003` |
| P3 | Refresh stale MCP ownership reference docs after implementation lands | Architecture | proposal author | Follow-on docs cleanup | final implementation truth | reference docs stop pointing to legacy `mcp_profile` as canonical | repo contradiction |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Failed-stage evidence namespace | uniqueness of persisted evidence artifacts across loop iteration and retry | artifact row path or name includes stage execution identity or iteration | no overwrite when the same logical stage fails again later in the run | next proposal review | hold if the path still only uses `stage_id + attempt_number` |
| MCP report ownership | ability to persist `profile_id` from execution truth | Rust `ResolvedAgent` carries backend profile identity | no YAML re-read in report/executor path | next proposal review | hold if `profile_id` is still implicit or ambiguous |
| Failed-stage packet schema honesty | whether `failed_agent_title` has a durable owner or explicit nullability rule | file inventory and acceptance text mention it directly | no silent best-effort field that looks durable but is not | next proposal review | hold if the field remains unspecified |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- GAP-01: No blocking evidence gap remains. Local proposal/docs/code/baseline evidence is sufficient for a proposal-readiness verdict.

### Open Questions
- QUESTION-01: Is `McpResolutionReport.profile_id` intended to be backend profile ID, runtime profile ID, or another identifier? The current Swift parity signal points to backend profile ID.
- QUESTION-02: Does the author want `failed_agent_title` parity in Rust V1, or is `null` acceptable until a later owner slice?

## 10. Evidence Gap Review Fallback

Not used. Evidence completeness is `Complete`.
