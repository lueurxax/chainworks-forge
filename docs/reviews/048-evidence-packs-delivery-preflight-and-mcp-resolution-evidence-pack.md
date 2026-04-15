# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md` | 2026-04-15 | High | The current draft closes the stale blockers from the prior round: `P046` is now an explicit dependency, missing/disabled requested MCP servers are now fail-closed, and the canonical ACP registry path / override / legacy migration are now named directly. Three new blockers remain: failed-stage packet substrate, runtime-scoped MCP resolver inputs, and delivery-preflight JSON shape drift. | Review could keep repeating stale blockers and miss the current live ones. | Primary proposal source. |
| DOC-02 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Repo-backed delivery and operator/report surfaces remain baseline behavior, not speculative future scope. | Review could misclassify run-start validation and report readers as optional future-state concerns. | Intake baseline. |
| DOC-03 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | 2026-04-15 | High | Stable failure evidence remains stage-owned and should be referenced through canonical evidence rather than inferred from loose scans. | Proposal can still overclaim parity if it does not name the owners that actually feed the packet. | Failed-stage evidence authority. |
| DOC-04 | `docs/reference/acp-runtime-transport.md` | 2026-04-15 | High | ACP runtime truth is transport-neutral, but runtime selection and adapter realization still stay bound to the selected runtime family. | Proposal can under-specify runtime-scoped MCP resolution and still claim parity. | Runtime boundary authority. |
| DOC-05 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Current Rust seams are concrete: command handler, executor, ACP transport, GraphQL, MCP server, and SQLite repos. | Proposal must stay grounded in the current crate/module owners. | Rust daemon boundary. |
| DOC-06 | `docs/proposals/046-structured-output-envelope-and-contract-validation.md` | 2026-04-15 | High | `P046` owns `ValidationFailureRecord` and envelope-derived validation substrate, but not the full failed-stage packet source-field set. | The dependency is now explicit, but the packet still needs additional owners. | Dependency boundary. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | intake baseline | 2026-04-15 | High | Fresh enough as intake only. | Entry baseline. |
| BASE-02 | `docs/reference/output-contracts-failure-evidence-and-recovery.md` | Partially refreshed | failed-stage evidence and recovery packet continuity | 2026-04-15 | High | Refreshed narrowly against current Swift and Rust owner seams. | Failure-evidence authority. |
| BASE-03 | `docs/reference/acp-runtime-transport.md` | Partially refreshed | runtime selection and ACP realization boundary | 2026-04-15 | High | Refreshed narrowly against current Swift MCP resolution and Rust ACP transport. | MCP/runtime authority. |
| BASE-04 | `docs/reference/rust-control-plane.md` | Partially refreshed | northbound and execution-owner topology | 2026-04-15 | High | Refreshed narrowly against current command/executor/db/server sources. | Daemon topology authority. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - stage-owned failed-stage evidence in Rust
  - run-creation delivery preflight for frozen delivery config
  - ACP `mcpServers` realization from `backend_profile.mcp`
- Out of scope:
  - shell-owned evidence-pack export
  - release-time readiness heuristics
  - implementation audit or gate execution
- Assumptions:
  - review mode is `proposal-readiness`
  - `P048` still claims parity with the current stable Swift owner chain unless it explicitly documents a contract change
  - the proposal is judged against the current working tree, not an older review cache
- Blockers:
  - the proposal does not yet name the Rust owners needed to build the full failed-stage packet it promises
  - the MCP resolver contract still omits runtime-scoped inputs and live registry loading semantics
  - the delivery preflight JSON schema drifts from the stable frozen payload contract

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | failed-stage report / recovery readers | Baseline + current repo | 2026-04-15 | High | Stable readers expect a stage-owned packet with richer source-field ownership than current Rust persists. | Proposal can still sound complete while leaving the packet unbuildable. | Critical failed-stage seam. |
| NAV-02 | runtime MCP validation and ACP session setup | Baseline + current repo | 2026-04-15 | High | Stable MCP realization depends on selected runtime namespace and fresh registry snapshots during preflight/session creation. | Proposal can still overclaim parity with an under-scoped resolver contract. | Critical MCP seam. |
| NAV-03 | delivery preflight report and start-block reasons | Baseline + current repo | 2026-04-15 | High | Stable UI and evidence/export flows already assume `{id, label, passed, detail}` for frozen delivery preflight results. | Proposal can still drift a persisted contract without naming a migration. | Delivery-preflight seam. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/acp/src/lib.rs` and `control-plane/crates/domain/src/agent.rs` | Rust runtime/domain | current ACP result + agent execution owner shape | 2026-04-15 | High | Current ACP result and agent execution rows do not persist the full failed-stage packet inputs the proposal promises. | Packet parity can still be overclaimed. | Critical blocker evidence. |
| MAP-02 | `control-plane/crates/domain/src/stage.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/domain/src/validation.rs` | Rust execution | current stage settlement + validation-failure substrate | 2026-04-15 | High | Rust currently persists validation-failure records, but not the full packet inputs like recovery snapshot, canonical outcome, or transport error kind. | The proposal can still miss required owner fields. | Critical blocker evidence. |
| MAP-03 | `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift` | Swift stable | canonical full failed-stage packet owner | 2026-04-15 | High | Stable packet parity is broader than the current Rust durable substrate. | Proposal cannot claim full parity without naming matching Rust owners. | Critical blocker evidence. |
| MAP-04 | `Chainworks Forge/Engine/MCPPolicyRuntime.swift` and `Chainworks Forge/Engine/RuntimeSessionBridge.swift` | Swift stable | canonical MCP resolution + ACP realization | 2026-04-15 | High | Stable MCP resolution depends on provider binding/runtime namespace and reloads the registry snapshot during validation and realization. | Proposal can still leave runtime-scoped MCP truth under-specified. | High blocker evidence. |
| MAP-05 | `Chainworks Forge/Engine/PreflightService.swift` | Swift stable | current MCP preflight registry loading | 2026-04-15 | High | Stable preflight reads the runtime registry snapshot at validation time, not only at process startup. | Proposal can weaken live-registry semantics. | High blocker evidence. |
| MAP-06 | `Chainworks Forge/Engine/DeliveryPreflightService.swift` | Swift stable | canonical persisted delivery-preflight result shape | 2026-04-15 | High | Stable `DeliveryPreflightService.PreflightCheck` uses `id` and `label`, not `name`. | Proposal can silently drift the frozen JSON contract. | Medium blocker evidence. |
| MAP-07 | `Chainworks Forge/Views/DeliveryPreflightReportView.swift`, `Chainworks Forge/Views/IdeaListView.swift`, `Chainworks Forge/Engine/EvidencePackBuilder.swift` | Swift stable | downstream readers of frozen delivery-preflight payload | 2026-04-15 | High | Current UI and evidence/export flows depend on the stable `id` / `label` fields. | Contract drift can break or complicate reuse. | Medium blocker evidence. |
| MAP-08 | `control-plane/crates/engine/src/command_handler.rs`, `control-plane/crates/domain/src/run.rs`, `control-plane/crates/db/src/repos/runs.rs` | Rust command/run persistence | start-run and frozen run JSON owner seam | 2026-04-15 | High | Delivery config already flows through the command/run owner lane, so the proposal targets the right start-run boundary. | Old run-start owner blocker is stale. | Confirmed seam. |
| MAP-09 | `control-plane/crates/acp/src/transport.rs` | Rust ACP | current `mcpServers: []` injection seam | 2026-04-15 | High | ACP transport still hardcodes `mcpServers: []`, so the proposal still targets a real missing capability. | The MCP seam itself is real; only the contract around it is incomplete. | Confirmed seam. |

## F. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | failed-stage packet source-field owners | Baseline + current repo | 2026-04-15 | High | Current Rust durable owners do not yet cover the full packet shape the proposal promises. | Proposal still overclaims parity. | Critical blocker. |
| INT-02 | runtime-scoped MCP resolution | Baseline + current repo | 2026-04-15 | High | Stable MCP resolution depends on selected runtime namespace and live registry snapshots. | Proposal still leaves both points implicit or weaker. | High blocker. |
| INT-03 | frozen delivery-preflight JSON | Baseline + current repo | 2026-04-15 | High | Stable preflight payload fields are `id` and `label`, and current readers use them directly. | Proposal still drifts the contract. | Medium blocker. |

## G. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Explicit `P046` dependency for validation/envelope substrate | Specified | DOC-01, DOC-06 | proposal depends-on line | Old undeclared dependency blocker is closed. |
| Missing/disabled requested MCP servers fail closed | Specified | DOC-01, MAP-04, MAP-05 | proposal AC 11 + current Swift behavior | Old fail-policy blocker is closed. |
| Canonical ACP registry path / override / Goose-era migration | Specified | DOC-01, MAP-04 | proposal lines 227-263 | Old registry-path blocker is closed. |
| Runtime-scoped MCP resolution input contract | Partial | MAP-04, MAP-05, INT-02 | proposal resolver signature vs stable runtime owner chain | High live blocker remains. |
| Live registry snapshot timing for preflight/session creation | Partial | MAP-04, MAP-05, INT-02 | proposal startup load wording vs current Swift reads | High live blocker remains. |
| Full failed-stage packet source-field ownership | Contradicted by repo | MAP-01, MAP-02, MAP-03, INT-01 | packet contract vs current Rust durable rows | Critical live blocker remains. |
| Delivery preflight persisted JSON shape | Contradicted by repo | MAP-06, MAP-07, INT-03 | proposal struct shape vs stable service/UI/export contract | Medium live blocker remains. |

## H. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | proposal acceptance | failed-stage evidence parity | current ACs require a full packet | ACs need the missing owner-field inventory, not just the packet type | 2026-04-15 | High | Critical blocker remains. |
| TEST-02 | proposal acceptance | runtime-scoped MCP resolution | current ACs cover fail-closed missing-server behavior and canonical path | ACs should also prove runtime-namespace-sensitive realization and live registry reload semantics | 2026-04-15 | High | High blocker remains. |
| TEST-03 | proposal acceptance | delivery preflight parity | current ACs block run start on failure | ACs should keep the stable persisted result schema, not rename it implicitly | 2026-04-15 | High | Medium blocker remains. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | failed-stage packet parity | Rust can port the full stable packet in the named seams | current Rust durable owners do not persist most packet source fields | 2026-04-15 | High | One critical blocker remains. |
| REAL-02 | runtime-scoped MCP resolution | `resolve_mcp_servers(requested_ids, registry)` is enough for parity | stable Swift resolution depends on provider binding/runtime namespace and fresh registry snapshots | 2026-04-15 | High | One high blocker remains. |
| REAL-03 | delivery-preflight persisted shape | the proposal ports `DeliveryPreflightService` | stable service and readers use `id` / `label`, not `name` | 2026-04-15 | High | One medium blocker remains. |
| REAL-04 | undeclared `P046` dependency | `P048` still relied on `ValidationFailureRecord` / envelopes implicitly | current draft now depends on `P046` explicitly | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-05 | fail-closed MCP policy | proposal still lets missing requested MCP servers proceed | current draft now makes the path fail-closed | 2026-04-15 | High | Earlier blocker is stale. |
| REAL-06 | registry contract ownership | proposal still invents a new MCP registry path | current draft now names the canonical ACP registry contract and Goose migration seam | 2026-04-15 | High | Earlier blocker is stale. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, MAP-08, MAP-09 | The targeted gaps remain real. |
| Scope boundaries | Specified | DOC-01, DOC-02 | Scope is substantially cleaner than the stale review basis assumed. |
| Reusable baseline coverage | Partial | BASE-02, BASE-03, BASE-04 | The old blockers are closed, but three new owner-contract gaps remain. |
| Data / execution contract | Contradicted by repo | MAP-01, MAP-02, REAL-01 | Failed-stage packet parity still overclaims current Rust substrate. |
| Runtime / MCP contract | Partial | MAP-04, MAP-05, REAL-02 | Runtime-scoped inputs and reload timing remain under-specified. |
| Reader / operator semantics | Partial | MAP-06, MAP-07, REAL-03 | Delivery-preflight frozen JSON contract still drifts. |
| Dependencies / integration points | Specified | DOC-06, REAL-04 | The earlier undeclared `P046` blocker is closed. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: `P048` still intends literal parity with the stable Swift failed-stage packet unless it explicitly narrows the packet.
- ASSUMP-02: `P048` still intends MCP parity with the current runtime-scoped resolver and live registry semantics unless it explicitly documents a behavior change.
- ASSUMP-03: the `DeliveryPreflightResult.PreflightCheck` example fields are intended to be normative, not placeholder pseudocode.
- QUESTION-01: should `P048` add the missing Rust owner fields needed for full failed-stage packet parity, or explicitly scope the packet down?
- QUESTION-02: should MCP registry reads remain live per preflight/session, or is startup-cached registry loading an intentional change?
- QUESTION-03: was the delivery-preflight `{ name }` field intentional, or should it revert to stable `{ id, label }`?
- BLOCKER-01: failed-stage packet parity still overclaims current Rust durable owners.
- BLOCKER-02: MCP resolution still omits runtime-scoped resolver inputs and live registry timing.
- BLOCKER-03: delivery-preflight persisted JSON shape still drifts from the stable contract.

## L. Research Triggers / External Questions
No external research trigger is required. Local proposal/docs/code/baseline evidence are sufficient for a proposal-readiness verdict.
