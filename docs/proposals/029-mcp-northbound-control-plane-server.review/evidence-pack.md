# Proposal Evidence Pack

Proposal: `docs/proposals/029-mcp-northbound-control-plane-server.md`
Mode: `proposal-readiness`
Verified on: 2026-04-16
Git SHA: `af3054c73064b05e42cb816a81a3c5fb0c2e29d9`
Working tree: Dirty; broad control-plane implementation files, P029 proposal/review artifacts, P048/P049 artifacts, and untracked P049 Steward files are present.

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/029-mcp-northbound-control-plane-server.md` | 2026-04-16 | High | Current draft is R6 and now specifies type ownership, auth on MCP/GraphQL, capability filtering, command journaling, GraphQL coexistence, dogfood migration, and `proposal-029-mcp`. | Review could carry stale blockers from an older draft. | Primary review target. |
| DOC-02 | Prior `029...review/evidence-pack.md` and `proposal-readiness-review.md` | 2026-04-16 | High | Prior artifacts are stale: R6 closes previous findings around type ownership, Stage A wording, SwiftUI GraphQL migration, WS unknown-token proof, and bootstrap token handling. | Carrying old findings would block already-fixed proposal text. | Freshness control. |
| DOC-03 | `docs/reference/current-system-baseline.md` | 2026-04-16 | High | Current baseline now lists Forge Steward as implemented system-health analysis. | P029's "current MCP surface" can miss active Steward additions. | Baseline intake. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-16 | Medium | Stable Rust control-plane reference maps GraphQL/MCP baseline but is older than current P049 Steward northbound additions. | Baseline-only review would miss newest MCP surface. | Targeted refresh trigger. |
| DOC-05 | `docs/reference/test-gates.md` and `scripts/test-gate.sh` | 2026-04-16 | High | P049 gate/reference mention GraphQL, MCP tool, and `steward-analysis://` readback; `proposal-029-mcp` is still only proposed, not registered. | Proof lane scope can omit active northbound surfaces. | Test strategy. |
| DOC-06 | `.mcp.json` and `CLAUDE.md` | 2026-04-16 | High | Dogfood MCP config is still bare HTTP with no `Authorization` header; P029 correctly requires same-commit migration. | Landing auth without config/docs migration strands local dogfooding. | UX/dogfood. |
| DOC-07 | `docs/proposals/029-mcp-northbound-control-plane-server_IMPLEMENTATION_AUDIT_R1.md` | 2026-04-16 | Medium | Older implementation audit reflected an earlier tree. Current dirty tree now has partial auth/caller/journal scaffolding, so this audit is no longer complete current truth. | Review could misclassify current repo reality. | Prior artifact caution. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---:|---|---|---|
| BASE-01 | `docs/reference/current-system-baseline.md` | Reused | product/subsystem baseline | 2026-04-16 | High | Fresh enough to establish Steward as baseline current truth. | Baseline scope. |
| BASE-02 | `docs/reference/rust-control-plane.md` | Partially refreshed | GraphQL/MCP daemon architecture | 2026-04-16 | Medium | Useful for old northbound boundary but stale for Steward tools/resources. | Code mapping. |
| BASE-03 | Proposal-local prior review artifacts | Partially refreshed | prior finding freshness | 2026-04-16 | High | Used only to identify closed vs live findings. | Review hygiene. |
| BASE-04 | `docs/proposals/029-mcp-northbound-control-plane-server.review/integration-context.md` | Missing | proposal-local reusable context | 2026-04-16 | High | Not blocking; targeted code refresh was narrow enough. | Optional context. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope: principal resolution for MCP HTTP, MCP stdio, GraphQL HTTP, and GraphQL WS; caller-scoped tool/resource filtering; caller metadata on `command_journal`; engine-owned payload redaction; `journal_id` surfacing; GraphQL coexistence; dogfood MCP auth migration.
- Out of scope: new tool/resource expansion, southbound agent MCP policy, token rotation/revocation/delegation, UI rewrite, GraphQL mutation removal, MCP protocol-version bump for `structuredContent`.
- Deferred intentionally: parent-process stdio identity, YAML-driven policy, future GraphQL mutation removal, future MCP structured content.
- Assumptions: this is proposal readiness, not implementation audit; current working tree is the source of "current repo reality" for surface inventory.
- Open questions: should P029 explicitly sequence after P049 and include Steward tool/resource capabilities, or should P029 require a clean pre-P049 baseline?
- Blockers: active Steward MCP/resource surface is missing from P029's current-surface inventory and capability/resource ID model.

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Surface / Entry Point | Source | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---:|---|---|---|---|
| NAV-01 | MCP HTTP `/mcp` | `mcp-server/src/http.rs:36-119` | 2026-04-16 | High | Current dirty tree already has bearer parsing and passes `Principal` into `McpServer::handle_request`. | Proposal/readiness could ignore partial implementation drift. | MCP auth seam. |
| NAV-02 | MCP stdio loop | `mcp-server/src/server.rs:43-170` | 2026-04-16 | High | Current dirty tree has initialize-based principal binding, but behavior differs from R6 details. | Implementation branch may not match proposal. | stdio auth seam. |
| NAV-03 | GraphQL `/graphql` | `graphql-server/src/server.rs:25-35`, `graphql-server/src/auth_layer.rs:1-66` | 2026-04-16 | High | Auth middleware file exists, but `start_with_extra_routes` does not mount it yet. | Current implementation is partial. | GraphQL auth seam. |
| NAV-04 | GraphQL `/graphql/ws` | `graphql-server/src/server.rs:26-35`, P029 §4.1.c | 2026-04-16 | High | WS auth is specified in proposal; current server still mounts plain `GraphQLSubscription::new(schema)`. | Implementation must add on-connection auth. | WS auth seam. |
| NAV-05 | MCP `tools/list` / `tools/call` | `mcp-server/src/server.rs:194-242`, `tools/steward.rs:10-46` | 2026-04-16 | High | Active tool registry includes `steward.*`, but P029's tool inventory/capability IDs do not. | P029 can hide or deny active tools after auth lands. | Critical surface drift. |
| NAV-06 | MCP `resources/list` / `resources/read` | `mcp-server/src/server.rs:244-345`, `server.rs:460-471` | 2026-04-16 | High | Active resource set includes `steward-analysis://{analysis_id}`, but P029's ResourceTemplateId list does not. | Resource auth can block Steward readback. | Critical surface drift. |
| NAV-07 | Local MCP dogfood config | `.mcp.json`, `CLAUDE.md:47-57` | 2026-04-16 | High | Current config/docs still document unauthenticated HTTP. | Same-commit migration remains mandatory. | Dogfood UX. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|---|
| MAP-01 | `control-plane/crates/mcp-server/src/tools/mod.rs:1-6` | MCP tools | module registry | 2026-04-16 | High | `steward` is now registered alongside approvals/ideas/reports/runs/stages. | P029's "currently registered tools" claim is stale. | Critical finding. |
| MAP-02 | `control-plane/crates/mcp-server/src/tools/steward.rs:10-46` | MCP tools | Steward tool specs | 2026-04-16 | High | Active tools are `steward.run_analysis`, `steward.list_analyses`, and `steward.get_analysis`. | Capability map omissions cause denied/hidden tools. | Critical finding. |
| MAP-03 | `control-plane/crates/mcp-server/src/server.rs:275-280`, `460-471` | MCP resources | Steward resource template/read | 2026-04-16 | High | Active resource URI is `steward-analysis://{analysis_id}`. | ResourceTemplateId omissions break readback. | Critical finding. |
| MAP-04 | `control-plane/crates/auth/src/lib.rs:12-18`, `158-171` | auth | current partial implementation | 2026-04-16 | High | Current dirty implementation keeps `PrincipalClass` in `auth` and filters string `ToolSpec`, while R6 proposes domain-owned typed IDs. | Implementation branch must be reconciled with R6. | Implementation drift. |
| MAP-05 | `control-plane/crates/domain/src/commands.rs:66-121` | domain | caller context | 2026-04-16 | High | Current dirty implementation uses `principal_class: String`, not `PrincipalClass`. | Current code does not match R6 owner graph. | Implementation drift. |
| MAP-06 | `control-plane/crates/daemon/src/main.rs:108-111` | daemon config | principal table load | 2026-04-16 | High | Current dirty code loads `principals.json` from cwd, not `CHAINWORKS_AUTH_PRINCIPALS_PATH` or `~/.chainworks/auth/principals.json`. | Implementation branch lags proposal. | Auth config seam. |
| MAP-07 | `scripts/test-gate.sh`, `docs/reference/test-gates.md` | tests | gate registry | 2026-04-16 | High | `proposal-029-mcp` is specified by P029 but not yet registered in current files. | Expected for proposal, but proof lane must land with implementation. | Test gate. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|
| DATA-01 | Typed auth ownership | P029 §4.0 lines 132-216 | domain/auth/server | 2026-04-16 | High | R6 now defines domain-owned `PrincipalClass`, `CapabilityToolId`, and `ResourceTemplateId`; old type-boundary finding is closed in proposal text. | Current implementation still needs rewrite. | Closed prior issue. |
| DATA-02 | Current tool capabilities | P029 §4.0 lines 155-156 and §4.2 lines 318-324 | capability policy | 2026-04-16 | High | P029 enum/table omits `steward.run_analysis`, `steward.list_analyses`, and `steward.get_analysis`. | Active tools can be inaccessible after P029. | Critical finding. |
| DATA-03 | Current resource capabilities | P029 §4.0 lines 156 and §6 lines 535-540 | resource policy | 2026-04-16 | High | P029 enum/table omits `steward-analysis://{analysis_id}`. | Active Steward readback can be inaccessible. | Critical finding. |
| DATA-04 | Command journal | P029 §4.3 lines 330-437; `engine/src/command_handler.rs:77-139` | persistence/audit | 2026-04-16 | High | R6 contract is clear; current dirty implementation already returns `Commanded`. | Steward command tool must be included in command-tool list if active. | Audit contract. |
| DATA-05 | GraphQL response wrapper migration | P029 §4.4.b lines 469-490 | API shape | 2026-04-16 | High | R6 corrects the SwiftUI consumer claim and scopes migration to tests/dev playground. | Prior finding is closed. | API migration. |
| DATA-06 | Bootstrap token handling | P029 §4.1 lines 222-231 and AC-15 | credential lifecycle | 2026-04-16 | High | R6 now requires 0600, one-time token log, env path, and fail-closed malformed/empty table. | Prior UX token finding is closed in proposal. | Auth UX. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---:|---|---|---|---|
| INT-01 | P049 Steward northbound surface | current code + baseline | 2026-04-16 | High | Current system includes Steward MCP tools and `steward-analysis://` readback. | P029 still defines current MCP surface as pre-Steward. | Critical finding. |
| INT-02 | Auth type owner graph | proposal + current dirty code | 2026-04-16 | High | Proposal owner graph is now compile-feasible, but current dirty code does not implement it. | Handoff from current tree requires cleanup, not incremental patching over old scaffold. | Readiness note. |
| INT-03 | GraphQL WS auth | proposal + current code | 2026-04-16 | High | R6 proposal is specific enough; current code has not mounted it. | Implementation gap, not proposal gap. | Closed prior blocker. |
| INT-04 | Dogfood MCP config | proposal + current config | 2026-04-16 | High | Proposal requires `.mcp.json` and `CLAUDE.md` migration; current files remain unauthenticated. | Landing must include the doc/config patch. | UX proof. |

## H. State Coverage Matrix
| State | Proposal Status | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| MCP HTTP unauthenticated | Specified | NAV-01, DATA-06 | `http.rs` | R6 contract is clear. |
| MCP HTTP authenticated happy path | Specified | NAV-01, INT-04 | `.mcp.json`, `http.rs` | Same-commit dogfood migration remains required. |
| MCP stdio pre-initialize / unauthenticated | Specified | NAV-02 | `server.rs`, `protocol.rs` | R6 has concrete failure semantics. |
| MCP stdio authenticated happy path | Specified | NAV-02 | `server.rs`, `protocol.rs` | R6 has a concrete token location. |
| GraphQL HTTP unauthenticated | Specified | NAV-03 | `server.rs`, `auth_layer.rs` | Current implementation partial, proposal sufficient. |
| GraphQL WS unauthenticated / unknown token / valid token | Specified | NAV-04, DATA-05 | `server.rs` | R6 includes all three test names. |
| `tools/list` filtering | Partial | NAV-05, DATA-02, INT-01 | `server.rs`, `tools/steward.rs` | Core behavior specified, but active Steward tools are missing from inventory. |
| `tools/call` denied path | Partial | NAV-05, DATA-02, INT-01 | `server.rs` | Omission affects active `steward.*` calls. |
| Command-tool audit | Partial | DATA-04, MAP-02 | `CommandHandler`, `tools/steward.rs` | P029 lists four command tools but active `steward.run_analysis` also invokes `CommandHandler`. |
| `resources/list` filtering | Partial | NAV-06, DATA-03 | `server.rs` | Active Steward resource missing from enum/table. |
| `resources/read` denied path | Partial | NAV-06, DATA-03 | `server.rs` | Template-instance matcher specified, but template inventory incomplete. |
| Rollback | Specified | DOC-01 | migration/auth wiring, `.mcp.json` | R6 rollback is concrete enough. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---:|---|---|
| FLAG-01 | `CHAINWORKS_AUTH_PRINCIPALS_PATH` | principal table loading | default path with bootstrap | keep nullable columns or revert auth wiring | 2026-04-16 | High | R6 specifies fail-closed empty string behavior. |
| FLAG-02 | `CHAINWORKS_PLAYGROUND_AUTH=skip` | GraphQL playground GET only | opt-in local playground bypass | unset env var | 2026-04-16 | High | Narrow and acceptable because POST/WS remain auth-scoped. |
| FLAG-03 | `CHAINWORKS_MCP_TOKEN` | dogfood MCP client credential | `.mcp.json` header expansion | revert `.mcp.json` to bare HTTP | 2026-04-16 | Medium | Proposal says supported by Claude Code; this round did not browse official docs. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---:|---|---|
| METRIC-01 | `command_journal.caller_*` | audit, caller attribution, GraphQL/MCP coexistence visibility | `CommandHandler::handle` | 2026-04-16 | High | Must include active `RunStewardAnalysis` command if P049 is in baseline. |
| METRIC-02 | GraphQL residual mutation traffic | future cutover signal | `caller_surface = 'graphql'` rows | 2026-04-16 | High | R6's Stage B plan is coherent. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---:|---|---|---|
| TEST-01 | Proposal proof lane | P029 auth/capability/journal | `proposal-029-mcp` proposed, not registered | add `PROPOSAL_029_MCP_TESTS` and `proposal-029-mcp|p029-mcp` | 2026-04-16 | High | Expected for proposal; must land with implementation. |
| TEST-02 | Active surface inventory | all current MCP tools/resources | P049 gate covers Steward readback separately | add P029 auth/capability assertions for `steward.*` and `steward-analysis://` if P049 remains current | 2026-04-16 | High | Current P029 test inventory omits active Steward surfaces. |
| TEST-03 | GraphQL WS auth | missing/unknown/valid token | proposal names all three tests | implement in P029 gate | 2026-04-16 | High | Prior WS test gap is closed in proposal. |
| TEST-04 | Audit contract | journal caller columns, redaction, `journal_id` | current dirty code has partial tests | proposal inventory covers core rows | 2026-04-16 | High | Add Steward command row if active. |
| TEST-05 | Dogfood MCP client | `.mcp.json` header/token path | current config has no auth | proposal requires config/docs update | 2026-04-16 | Medium | Local proof or official docs check may be needed at implementation time. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---:|---|---|
| REAL-01 | MCP tool inventory | Currently registered tools are approvals/ideas/reports/runs/stages only. | `mcp-server/src/tools/mod.rs` also registers `steward`; `tools/steward.rs` defines three `steward.*` tools. | 2026-04-16 | High | Proposal is stale against current tree. |
| REAL-02 | MCP resource inventory | ResourceTemplateId variants cover run/idea/artifact/report and chainworks collections only. | `mcp-server/src/server.rs` includes `steward-analysis://{analysis_id}` in list/read paths. | 2026-04-16 | High | Resource auth model incomplete. |
| REAL-03 | Command tool set | MCP command tools are `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`. | `steward.run_analysis` invokes `Command::RunStewardAnalysis` through `CommandHandler`. | 2026-04-16 | High | Journal ID/audit ACs omit an active command tool. |
| REAL-04 | Auth type ownership | R6 says domain owns `PrincipalClass`, `CapabilityToolId`, `ResourceTemplateId`; `auth` consumes typed IDs. | Current dirty implementation has `PrincipalClass` and string `ToolSpec` in `auth`; no `domain/src/capabilities.rs`. | 2026-04-16 | High | Implementation scaffold must be replaced to match proposal. |
| REAL-05 | GraphQL auth mount | R6 specifies GraphQL auth middleware and WS `on_connection_init`. | Current dirty server does not mount auth middleware or WS auth. | 2026-04-16 | High | Implementation gap, not proposal gap. |
| REAL-06 | Dogfood config | R6 requires same-commit `.mcp.json` header migration. | Current `.mcp.json` remains bare HTTP. | 2026-04-16 | High | Must be included when implementation lands. |

## M. Proposal Completeness Matrix
| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Delta posture and P031 blocker framing are clear. |
| Scope boundaries | Partial | REAL-01..REAL-03 | R6 fixed old direct-vs-command scope, but misses active Steward northbound surfaces. |
| Reusable baseline coverage | Partial | BASE-01, BASE-02, INT-01 | Current-system baseline includes Steward; P029 does not absorb it. |
| Screen / surface definition | Partial | NAV-01..NAV-07 | Backend surfaces mapped; active Steward surface omitted. |
| Navigation / entry points | Partial | NAV-05, NAV-06 | Auth entry points covered; tool/resource inventory stale. |
| State handling | Partial | H matrix | Main auth states covered; Steward auth states missing. |
| Data / API contract | Partial | DATA-01..DATA-06 | Type ownership strong in text; active IDs incomplete. |
| Persistence / caching | Partial | DATA-04, REAL-03 | Journal model clear; active Steward command missing from command-tool set. |
| Permissions / auth expiry | Partial | DATA-02, DATA-03 | Capability model incomplete for active surface. |
| Feature flags / rollout / rollback | Specified | FLAG-01..FLAG-03 | Rollback and envs are sufficient. |
| Analytics / instrumentation | Partial | METRIC-01 | Steward command audit not covered. |
| Testing strategy | Partial | TEST-01..TEST-05 | Missing Steward auth/capability/resource/journal proof if P049 remains current. |
| Dependencies / integration points | Partial | INT-01, INT-02 | Main unresolved issue is current northbound surface drift. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: R6 proposal text is authoritative over stale prior review findings.
- ASSUMP-02: Runtime execution was not required for proposal readiness; code inspection is sufficient for P029 seam mapping.
- ASSUMP-03: The dirty working tree's P049 Steward MCP/resource additions count as current repo reality for this review.
- QUESTION-01: Should P029 absorb P049 Steward tools/resources into first-wave capability policy, or should P029 be explicitly sequenced against a clean pre-P049 base?
- QUESTION-02: What capability class should own `steward.run_analysis` if absorbed: operator-only, or agent-visible manual analysis trigger?
- BLOCKER-01: P029's current-surface inventory omits active Steward tools/resource and therefore is not implementation-ready on this tree.

## O. Research Triggers / External Questions
No external research was run in this proposal-readiness round.

| Trigger ID | Trigger Type | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Host-system integration risk | FLAG-03, TEST-05 | Re-check Claude Code HTTP MCP `headers` plus environment expansion support if local dogfood proof fails during implementation. | Current round used repo-local evidence only. | Medium; client config behavior can change. |
