# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` | 2026-04-15 | High | The current draft already closes the stale blocker set: explicit `P046` dependency, stable delivery-preflight payload shape, canonical MCP registry path/override/migration, and fail-closed execution-time MCP semantics are all present. The remaining live issues are contract details around artifact identity and owner declaration. | Review could keep repeating stale blockers and miss the real remaining gaps. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Repo-backed runs, operator-visible recovery/report surfaces, and persisted evidence remain the baseline, not speculative future scope. | Review could treat persistence/readers as optional. | Intake baseline. |
| DOC-03 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-15 | High | Failed-stage evidence is stage-owned, reports should consume canonical evidence rather than loose scans, and same-run retry needs disjoint lineage-friendly namespaces. | Proposal can still under-specify failure-evidence identity and reader truth. | Failure-evidence authority. |
| DOC-04 | `docs/reference/workflow-execution-engine.md` | 2026-04-15 | High | Canonical artifact storage uses `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}`. | A flat `stage_id + attempt` path can look harmless while breaking namespace guarantees. | Artifact namespace authority. |
| DOC-05 | `docs/reference/acp-runtime-transport.md` | 2026-04-15 | High | Runtime-scoped MCP realization depends on the selected runtime/provider binding and machine-local registry truth. | Proposal can under-specify runtime-sensitive MCP resolution. | MCP/runtime authority. |
| DOC-06 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-15 | High | Stage truth, recovery truth, and canonical outcome/evidence ownership remain explicit lower-layer contracts. | Proposal can overclaim packet parity if owner seams are not named. | Recovery/settlement authority. |
| DOC-07 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | 2026-04-15 | High | This reference doc is partially stale: it still centers legacy MCP ownership while current code uses `backend_profile.mcp`. | Review must prefer working-tree code over stale reference wording. | Freshness warning. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | repo-backed run baseline and operator/report framing | 2026-04-15 | High | Fresh enough as intake. | Review entry baseline. |
| BASE-02 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | Partially refreshed | failed-stage evidence and retry-lineage rules | 2026-04-15 | High | Refreshed against current Rust/Swift owner seams. | Failure-evidence baseline. |
| BASE-03 | `docs/reference/workflow-execution-engine.md` | Partially refreshed | canonical artifact namespace and loop topology | 2026-04-15 | High | Refreshed against current orchestrator and retry code. | Namespace baseline. |
| BASE-04 | `docs/reference/acp-runtime-transport.md` | Partially refreshed | runtime-scoped MCP realization boundary | 2026-04-15 | High | Refreshed against current Swift MCP policy/runtime bridge and Rust ACP transport. | MCP baseline. |
| BASE-05 | `docs/reference/per-agent-mcp-policy-and-runtime-validation.md` | Partially refreshed | MCP ownership language | 2026-04-15 | High | Use with caution; working-tree code is newer than this doc for MCP ownership. | Freshness caveat. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - stage-owned failed-stage evidence in Rust
  - run-start delivery preflight persistence/blocking
  - ACP `mcpServers` realization from `backend_profile.mcp`
- Out of scope:
  - shell-owned evidence-pack export
  - release-time readiness heuristics
  - implementation audit or proof execution
- Deferred intentionally:
  - start-time `PreflightService` MCP warning parity
  - full V2 failed-stage packet parity for currently nullable fields
- Assumptions:
  - review mode is `proposal-readiness`
  - `profile_id` is intended to align with Swift `backendProfileID` semantics
  - proposal is judged against current working-tree reality, not a stale cached review
- Open questions:
  - should Rust V1 carry `failed_agent_title`, or explicitly leave it `null`
  - is `profile_id` definitely backend profile ID
- Blockers:
  - canonical failed-stage evidence identity is still under-specified
  - MCP `profile_id` still lacks a named Rust owner
  - `failed_agent_title` still lacks a named owner or explicit deferral note

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | delivery-preflight report and run-start gate (`DeliveryPreflightReportView`, `IdeaListView.startRun`) | Baseline + current repo | 2026-04-15 | High | Stable readers use `check.id` and `check.label`, and run start is blocked when delivery preflight fails. | Proposal review could falsely keep the old payload-shape blocker alive. | Confirms old blocker is closed. |
| NAV-02 | failed-stage evidence and recovery readers (`FailedStageEvidencePanel`, `RecoverySheet`, `RunReportBuilder`) | Baseline + current repo | 2026-04-15 | High | Operator readers consume stage-owned packet truth and care about title/summary/evidence continuity. | Owner omissions can degrade operator-facing packet clarity. | Failed-stage packet relevance. |
| NAV-03 | MCP report/comparison readers (`RunReportBuilder`, `RunComparisonService`) | Baseline + current repo | 2026-04-15 | High | Stable MCP policy reporting includes `profileID` plus requested/predicted/denied truth. | Proposal can still under-specify owner fields even when the high-level flow is right. | MCP report relevance. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/engine/src/orchestrator.rs:824-839`, `control-plane/crates/engine/src/command_handler.rs:336-345`, `docs/reference/workflow-execution-engine.md:175` | Rust orchestration + baseline | loop iteration, same-stage retry, and canonical artifact namespace | 2026-04-15 | High | New loop iteration resets `attempt_number` to `1`, while same-stage retry increments `attempt_number`; canonical artifact storage includes iteration to avoid collisions. | A flat `stage_id + attempt` artifact identity is not safe. | Supports `ARCH-001`. |
| MAP-02 | `Chainworks Forge/Engine/EvidencePackBuilder.swift:160-166` | Swift export layer | existing failure-evidence export naming | 2026-04-15 | High | Swift currently uses flat `evidence-<stageID>-attempt<n>.json` only inside export-pack assembly. | Proposal can accidentally promote export naming into canonical runtime storage. | Supports `ARCH-001`. |
| MAP-03 | `control-plane/crates/workflow/src/plan.rs:41-66`, `control-plane/crates/workflow/src/compiler.rs:76-87`, `control-plane/crates/workflow/src/catalog.rs:62-72,104-122` | Rust compiler/plan | current resolved-agent owner shape | 2026-04-15 | High | Current Rust `ResolvedAgent` lacks title and backend profile ID; compiler `AgentBinding` also lacks both. | Proposal fields can overclaim available Rust truth. | Supports `ARCH-002`, `ARCH-003`. |
| MAP-04 | `Chainworks Forge/Engine/RunPlan.swift:149-179`, `Chainworks Forge/Engine/RunPlanCompiler.swift:231-255`, `Chainworks Forge/Engine/MCPPolicyRuntime.swift:254-295`, `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift:66-76` | Swift stable | parity owner chain for MCP `profileID` and failed agent title | 2026-04-15 | High | Stable Swift `ResolvedAgent` carries both `backendProfileID` and `title`, and downstream builders use them directly. | Rust port can miss parity-critical owner propagation. | Supports `ARCH-002`, `ARCH-003`. |
| MAP-05 | `control-plane/crates/acp/src/lib.rs:13-55`, `control-plane/crates/acp/src/transport.rs:502-507` | Rust ACP transport | current MCP handoff gap | 2026-04-15 | High | Rust ACP request/result currently have no MCP fields and transport still sends `"mcpServers": []`. | Confirms the proposal still targets a real missing capability. | Confirms MCP scope is valid. |
| MAP-06 | `Chainworks Forge/Engine/DeliveryPreflightService.swift:9-24`, `Chainworks Forge/Views/DeliveryPreflightReportView.swift:34-74`, `Chainworks Forge/Views/IdeaListView.swift:2241-2248` | Swift stable | canonical delivery-preflight frozen contract and gate | 2026-04-15 | High | Stable shape is `{ id, label, passed, detail }` and the start gate already consumes it. | Review could keep a stale blocker alive if this is missed. | Confirms old blocker is closed. |
| MAP-07 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift:254-295`, `Chainworks Forge/Engine/RuntimeSessionBridge.swift:135-171` | Swift stable | canonical MCP report ownership and runtime resolution | 2026-04-15 | High | Stable MCP policy reporting derives `profileID` from `agent.backendProfileID` and resolves against a live runtime registry snapshot. | Proposal needs a matching owner in Rust if it wants the same field. | Supports `ARCH-002`. |
| MAP-08 | `control-plane/crates/domain/src/stage.rs:84-104`, `control-plane/crates/domain/src/agent.rs:41-58`, `control-plane/crates/db/src/repos/stages.rs`, `control-plane/crates/db/src/repos/agent_executions.rs` | Rust domain/persistence | current durable owner seams | 2026-04-15 | High | Current stage/agent execution records do not carry failed agent title and do not yet expose proposed MCP report fields. | The proposal must name added owners or explicit deferrals. | Supports `ARCH-002`, `ARCH-003`. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | `stage_executions.evidence_packet_json` | proposal + `domain/src/stage.rs` + `db/src/repos/stages.rs` | write/read | 2026-04-15 | High | Proposal correctly introduces stage-owned packet persistence, but artifact identity remains under-specified. | Packet can persist while report artifact path still collides. | Failed-stage evidence persistence. |
| DATA-02 | `runs.delivery_preflight_json` | proposal + `domain/src/run.rs` + `db/src/repos/runs.rs` | write/read | 2026-04-15 | High | Proposal places delivery preflight on the right run-start persistence seam. | Old start-run owner blocker would be stale if restated. | Delivery preflight persistence. |
| DATA-03 | agent-execution MCP truth columns | proposal + `domain/src/agent.rs` + `db/src/repos/agent_executions.rs` | write/read | 2026-04-15 | High | Proposal introduces the right persistence lane for requested/predicted/actual/denied MCP truth, but `profile_id` ownership is still missing from the plan/compiler side. | MCP report persistence can stay partially implicit. | MCP truth persistence. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | failed-stage evidence readers and report/export surfaces | Baseline + current repo | 2026-04-15 | High | Canonical evidence is stage-owned; export bundles are downstream packaging, not the canonical artifact namespace. | Proposal currently conflates export naming with canonical runtime artifact storage. | Supports `ARCH-001`, `ARCH-003`. |
| INT-02 | runtime-scoped MCP resolution | Baseline + current repo | 2026-04-15 | High | Execution-time MCP resolution is the right owner boundary, but stable profile reporting depends on backend profile ID being present on the resolved agent. | Proposal still needs an explicit Rust owner for `profile_id`. | Supports `ARCH-002`. |
| INT-03 | frozen delivery-preflight result | Baseline + current repo | 2026-04-15 | High | Proposal now preserves the stable frozen JSON shape and the run-start block semantics. | No live blocker remains here. | Confirms stale finding is closed. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Delivery preflight passes and persists frozen result | Specified | `DOC-01`, `MAP-06`, `DATA-02`, `INT-03` | proposal lines 137-186; Swift stable preflight readers | Old payload-shape/start-gate blocker is closed. |
| Delivery preflight fails and blocks run start | Specified | `DOC-01`, `MAP-06`, `INT-03` | proposal lines 165-186; `IdeaListView.startRun` | Stable semantics preserved. |
| Failed stage settles and packet persists on stage row | Specified | `DOC-01`, `DOC-03`, `DATA-01` | proposal lines 68-130 | Stage-owned persistence lane is correctly targeted. |
| Failed-stage evidence artifact identity across loop iteration changes | Partial | `DOC-01`, `DOC-04`, `MAP-01`, `MAP-02`, `INT-01` | proposal lines 123-125, 404 | Current draft still allows collisions / canonical-path confusion. |
| Same-stage retry within one iteration | Partial | `DOC-03`, `MAP-01` | `command_handler.rs` retry stage creation | Namespace must stay unique here too. |
| MCP registry missing or disabled at executor boundary | Specified | `DOC-01`, `DOC-05`, `MAP-05`, `INT-02` | proposal lines 253-305, 353-362 | Old fail-closed blocker is closed. |
| MCP runtime/profile report owner truth | Partial | `DOC-01`, `MAP-03`, `MAP-04`, `MAP-07`, `DATA-03` | proposal lines 213-220, 307-324 | `profile_id` owner remains underspecified. |
| Failed-stage packet agent title truth | Partial | `DOC-01`, `MAP-03`, `MAP-04`, `MAP-08` | proposal packet struct and immediate settlement text | Title needs an owner or explicit nullability rule. |
| Start-run MCP warning parity | Deferred intentionally | `DOC-01`, `DOC-05`, `MAP-07` | proposal lines 259-305 | Explicitly out of scope for this slice. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | none proposed | internal control-plane slice | land behind normal proposal acceptance/proof gates | revert proposal/implementation change set if needed | 2026-04-15 | Medium | No special feature flag is required for this infrastructure slice. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | persisted failed-stage evidence artifacts and MCP report rows | operator/report observability instead of ad hoc logs | stage settlement and agent execution persistence | 2026-04-15 | Medium | The proposal should prove artifact uniqueness and field ownership; otherwise observability stays ambiguous. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal acceptance | failed-stage evidence namespace | current AC mentions the flat artifact path only | add proof that iteration 1 / attempt 1 and iteration 2 / attempt 1 produce distinct persisted evidence artifacts | 2026-04-15 | High | Without this, the core namespace gap can slip through. |
| TEST-02 | proposal acceptance | delivery preflight frozen contract | current ACs already preserve payload shape and start-block semantics | keep as-is | 2026-04-15 | High | This is no longer a live blocker. |
| TEST-03 | proposal acceptance | MCP report ownership | current ACs cover runtime resolution/fail-closed behavior | add proof that `profile_id` is emitted from persisted execution truth | 2026-04-15 | High | Report field can remain under-owned otherwise. |
| TEST-04 | proposal acceptance | failed agent title contract | not explicitly covered | add proof that `failed_agent_title` is either persisted or explicitly `null` in V1 | 2026-04-15 | High | Packet schema can remain misleading otherwise. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | prior review blocker set | old blockers still apply | current draft already fixes explicit `P046`, delivery-preflight payload shape, and canonical MCP registry/fail-closed semantics | 2026-04-15 | High | old review artifacts are stale and should not drive the current verdict |
| REAL-02 | MCP ownership reference docs | legacy MCP ownership wording is still authoritative | current working-tree code uses `backend_profile.mcp` as canonical MCP owner chain | 2026-04-15 | High | baseline freshness is partial; prefer code truth |
| REAL-03 | failed-stage evidence artifact path | proposal path is "canonical" and safe | current repo canonical artifact model includes iteration; proposal path does not | 2026-04-15 | High | live architecture blocker |
| REAL-04 | MCP `profile_id` | proposal can persist `profile_id` from its planned Rust changes | current proposed Rust file inventory does not add backend profile identity to `ResolvedAgent` | 2026-04-15 | High | live architecture gap |
| REAL-05 | `failed_agent_title` | packet field exists and can be filled at settlement | current Rust durable owner chain does not carry title | 2026-04-15 | High | live architecture gap |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01`, `DOC-02`, `MAP-05`, `MAP-06` | The proposal still targets real missing capabilities. |
| Scope boundaries | Specified | `DOC-01`, `DOC-02` | Scope is tighter and more honest than the stale review assumed. |
| Reusable baseline coverage | Partially refreshed | `BASE-01`, `BASE-02`, `BASE-03`, `BASE-05` | Enough for a defensible call, but one MCP reference doc is stale. |
| Screen / surface definition | Specified | `NAV-01`, `NAV-02`, `NAV-03` | Relevant reader surfaces are clear. |
| Navigation / entry points | Specified | `NAV-01`, `MAP-06` | Run start, recovery, and report surfaces are identifiable. |
| State handling | Partial | `H` matrix, `MAP-01`, `MAP-08` | Namespace and owner edge cases remain. |
| Data / API contract | Partial | `DATA-01`, `DATA-03`, `REAL-03`, `REAL-04`, `REAL-05` | Failure-evidence and MCP report contracts still need tightening. |
| Persistence / caching | Partial | `DATA-01`, `DATA-02`, `DATA-03` | Main persistence lanes are right, but two owner details and one namespace rule remain incomplete. |
| Permissions / auth expiry | Specified | `DOC-01` | No new auth surface is introduced in this slice. |
| Feature flags / rollout / rollback | Specified | `FLAG-01` | No special rollout mechanism is required. |
| Analytics / instrumentation | Partial | `METRIC-01`, `TEST-01`, `TEST-03`, `TEST-04` | Persistence-based observability is right, but proofs still need to lock identity/ownership. |
| Testing strategy | Partial | `TEST-01`, `TEST-02`, `TEST-03`, `TEST-04` | Three focused additions would close the remaining gaps. |
| Dependencies / integration points | Specified | `DOC-01`, `DOC-05`, `MAP-05`, `MAP-06` | Dependency and integration framing is materially better than the stale prior round. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `profile_id` is intended to match Swift `backendProfileID`, not runtime profile ID.
- ASSUMP-02: export-pack naming is not intended to redefine canonical runtime artifact storage.
- ASSUMP-03: operator readers still benefit from `failed_agent_title` if it remains in the packet schema.
- QUESTION-01: should `failed_agent_title` be in Rust V1 at all, or is explicit `null` acceptable?
- QUESTION-02: if `profile_id` is not backend profile ID, what exact identifier should reports persist?
- BLOCKER-01: failed-stage evidence artifact identity is still unsafe/incomplete for looped stages.
- BLOCKER-02: MCP `profile_id` still lacks a named Rust owner.
- BLOCKER-03: `failed_agent_title` still lacks a named owner or explicit deferral rule.

## O. Research Triggers / External Questions

No external research trigger is required. Local proposal/docs/code/baseline evidence is sufficient for a proposal-readiness verdict.
