# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/029-mcp-northbound-control-plane-server.md` | 2026-04-15 | High | Current R3 closes the stale blockers from the previous round: GraphQL principal resolution is now specified, stdio bootstrap is bound to `initialize.params.clientInfo.principal_token`, and audit storage extends `command_journal` instead of forking `mcp_audit_log`. Two proposal-level contradictions remain: audit privacy text still points to `mcp-server/src/audit.rs`, and auth helper ownership oscillates between a shared `auth` crate and `mcp-server/src/auth.rs`. | A stale review would keep already-closed blockers open and miss the real remaining contradictions. | Primary review target. |
| DOC-02 | `docs/proposals/029-mcp-northbound-control-plane-server.review/evidence-pack.md` and `.../proposal-readiness-review.md` from the previous round | 2026-04-15 | High | Existing local review artifacts are stale against current R3 and still describe blockers that current proposal text already answers. | Review output would misstate readiness and send the author back to already-closed issues. | Freshness control. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-15 | High | Proposal review should use stable current-system references first and only fall back to direct code mapping for surfaces not covered well enough by the baseline. | Proposal could be judged against stale proposal history instead of current stable references. | Required intake. |
| DOC-04 | `docs/reference/rust-control-plane.md` | 2026-04-15 | High | Stable daemon reference already matches the current MCP tool namespaces, entity URIs, run-scoped `chainworks://` collections, and HTTP + stdio transport split. | P029 must stay aligned to this existing server surface. | Primary MCP baseline. |
| DOC-05 | `scripts/test-gate.sh` and `docs/reference/test-gates.md` | 2026-04-15 | High | The repo already reserves `proposal-029|p029` for the ACP second-wave runtime lane, so P029's switch to `proposal-029-mcp` is the correct non-colliding proof-lane identity. | Carrying the old gate-collision finding forward would be incorrect. | Proof-lane freshness check. |
| DOC-06 | `control-plane/crates/mcp-server/src/{server,http,protocol}.rs`, `control-plane/crates/graphql-server/src/{server,schema}.rs`, `control-plane/crates/engine/src/command_handler.rs`, `control-plane/crates/db/src/repos/command_journal.rs` | 2026-04-15 | High | Current code confirms the intended owner seams that R3 now targets: GraphQL needs an auth/context layer, stdio today is raw JSON-RPC, and `command_journal` rows are still minted inside `CommandHandler`. | Proposal-readiness would be overstated if the remaining text is not checked against current owner reality. | Current repo reality. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review posture | 2026-04-15 | High | Fresh enough for intake. Narrow code refresh was still required for GraphQL, MCP, and journaling seams. | Review setup. |
| BASE-02 | `docs/reference/rust-control-plane.md` | Reused | MCP and GraphQL northbound baseline | 2026-04-15 | High | Fresh for tools, resources, and transports. Direct code refresh was needed for exact owner seams. | Host-system slice. |
| BASE-03 | prior P029 review artifacts | Partially refreshed | stale findings only | 2026-04-15 | High | Reused only to confirm which blockers R3 already closed. | Freshness boundary. |
| BASE-04 | `docs/proposals/029-mcp-northbound-control-plane-server.review/integration-context.md` | Missing | proposal-local reusable context | 2026-04-15 | High | No dedicated integration-context file exists. This did not block the round because the affected surfaces were narrow and directly inspectable. | Not a blocker. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - principal resolution for MCP HTTP and stdio,
  - caller-scoped capability filtering,
  - caller attribution in `command_journal`,
  - explicit GraphQL coexistence and later cutover rules,
  - alignment of proposal text to the already-implemented MCP surface.
- Out of scope:
  - new MCP tool families and deferred resource URIs,
  - southbound runtime policy,
  - token rotation and revocation beyond local principal records,
  - UI changes.
- Deferred intentionally:
  - Stage B and Stage C GraphQL cutover work,
  - future tool/resource expansion listed in proposal section 3.2.
- Assumptions for this round:
  - R3 is authoritative over the stale local review artifacts.
  - Stage B and Stage C are not readiness blockers for landing P029's Stage A scope.
  - `CommandHandler` remains the canonical mutating command and journaling owner.
- Open questions:
  - Does P029 really want client-visible `journal_id` in this slice, or is durable storage-only attribution enough?
  - Should `filter_tools` and `filter_resources` live directly in the shared `auth` crate or behind transport-local adapters?
- Current live findings discovered in this round:
  - The audit contract is still internally inconsistent: section 4.3 moves redaction into the engine, while risk 11.4 reintroduces `mcp-server/src/audit.rs::redact(...)`, and AC-11 promises client-facing `journal_id` exposure without defining the northbound response shape.
  - The auth/capability owner chain still drifts between `control-plane/crates/auth` and `mcp-server/src/auth.rs`.

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | MCP HTTP `/mcp` | Targeted refresh | 2026-04-15 | High | Current HTTP transport is the correct owner seam for bearer-token parsing and per-request principal resolution. | P029 HTTP auth could be reviewed against the wrong owner. | MCP HTTP auth seam. |
| NAV-02 | MCP stdio loop | Targeted refresh | 2026-04-15 | High | Current stdio server is raw JSON-RPC over stdin/stdout, so binding auth bootstrap to `initialize.params.clientInfo.principal_token` is compatible with current transport shape. | A false stale finding would keep calling stdio bootstrap ambiguous when it is now concrete. | MCP stdio auth seam. |
| NAV-03 | GraphQL `/graphql` and `/graphql/ws` | Targeted refresh | 2026-04-15 | High | Current GraphQL mount is the right owner seam for adding auth middleware and request-context principal injection. | Proposal-readiness would be misjudged if GraphQL were still treated as lacking any defined seam. | GraphQL auth and coexistence. |
| NAV-04 | MCP `tools/list`, `tools/call`, `resources/list` | Targeted refresh | 2026-04-15 | High | Capability filtering and caller-context propagation still land in `McpServer::handle_request`. | Auth helper ownership needs to be coherent because both tool and resource filtering depend on it. | Capability policy path. |
| NAV-05 | `command_journal` write path | Targeted refresh | 2026-04-15 | High | Current journal rows are created inside `CommandHandler::handle` from serialized `Command` values. | Any text that pushes redaction back into `mcp-server` contradicts the canonical writer path. | Audit owner path. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `control-plane/crates/mcp-server/src/http.rs` | MCP transport | HTTP request intake | 2026-04-15 | High | There is no bearer auth today, so proposal section 4.1.a correctly targets this file for principal resolution. | HTTP auth ownership would drift. | HTTP contract. |
| MAP-02 | `control-plane/crates/mcp-server/src/server.rs` | MCP server | stdio loop, dispatch, tool/resource list | 2026-04-15 | High | `run_stdio` and `handle_request` are the concrete owner seams for session-bound principal handling and capability filtering. | Capability policy could be assigned to the wrong layer. | stdio and filter contract. |
| MAP-03 | `control-plane/crates/mcp-server/src/protocol.rs` | Protocol | JSON-RPC `initialize` shape | 2026-04-15 | High | Current protocol layer is compatible with extending `clientInfo` for `principal_token`. | The old "ambiguous stdio bootstrap" finding no longer holds. | stdio protocol contract. |
| MAP-04 | `control-plane/crates/graphql-server/src/server.rs` | GraphQL transport | route mounting and middleware insertion | 2026-04-15 | High | Current mount has no auth layer today, so proposal section 4.1.c correctly names the insertion point. | GraphQL auth could still be reviewed as ownerless when it is no longer ownerless in proposal text. | GraphQL auth seam. |
| MAP-05 | `control-plane/crates/graphql-server/src/schema.rs` | GraphQL | mutation authoring path | 2026-04-15 | High | Mutation resolvers are the correct owner seam for reading `Principal` from context and constructing `CallerContext::graphql(...)`. | GraphQL caller attribution could be treated as unspecified when it is now concretely wired in proposal text. | GraphQL mutation path. |
| MAP-06 | `control-plane/crates/engine/src/command_handler.rs` | Engine | canonical command execution and journaling | 2026-04-15 | High | `handle()` still mints `journal_id`, serializes `Command`, writes the journal row, then executes. | Audit privacy text must stay aligned with this engine-owned path. | Audit plumbing. |
| MAP-07 | `control-plane/crates/db/src/repos/command_journal.rs` | Persistence | journal insert/complete/fail | 2026-04-15 | High | `record()` remains the single write path for persisted command audit rows. | A proposal that refers back to `mcp-server/src/audit.rs` would fork the audit story again. | Audit storage contract. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | GraphQL principal resolution | P029 section 4.1.c, `graphql-server/src/server.rs`, `graphql-server/src/schema.rs` | Inbound / auth | 2026-04-15 | High | R3 now names a concrete GraphQL auth middleware and context-injection seam, so the old GraphQL-auth blocker is closed. | A stale review would keep a closed blocker open. | Freshness control. |
| DATA-02 | stdio principal bootstrap | P029 section 4.1.b, `mcp-server/src/server.rs`, `protocol.rs` | Inbound / auth | 2026-04-15 | High | R3 binds stdio auth to `initialize.params.clientInfo.principal_token`, with precedence and failure semantics spelled out. | The old stdio-ambiguity blocker is closed and should not survive this round. | Freshness control. |
| DATA-03 | Audit redaction owner | P029 section 4.3 and risk 11.4, `command_handler.rs`, `command_journal.rs` | Internal persistence | 2026-04-15 | High | Section 4.3 correctly moves redaction into `engine/src/command_journal_redact.rs`, but risk 11.4 still says `mcp-server/src/audit.rs::redact(tool_name, args)` performs redaction before insert. | The proposal still contains two incompatible audit-owner stories. | Live finding. |
| DATA-04 | `journal_id` surfacing | P029 section 4.3 item 4 and AC-11 | Internal and northbound | 2026-04-15 | High | The proposal defines an internal `Commanded { result, journal_id }` wrapper but does not define the MCP result shape or GraphQL schema change needed for "callers that request one." | Implementers would have to invent the northbound audit-pointer contract. | Live finding. |
| DATA-05 | auth helper ownership | P029 section 4.1 versus section 4.2 and section 6 | Inbound / policy | 2026-04-15 | High | Section 4.1 places `resolve_bearer`, `filter_tools`, and `filter_resources` in a shared `control-plane/crates/auth` crate, but sections 4.2 and 6 refer back to `mcp-server/src/auth.rs`. | Shared capability policy ownership is still ambiguous. | Live finding. |
| DATA-06 | proof-lane identity | P029 section 9, `scripts/test-gate.sh`, `docs/reference/test-gates.md` | Verification | 2026-04-15 | High | The slug collision is actually resolved now: `proposal-029` remains ACP second-wave, and P029 uses `proposal-029-mcp`. | Old review output would remain stale if this were missed. | Freshness control. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | shared auth boundary | Proposal plus current repo | 2026-04-15 | High | Proposal text wants one shared auth/capability policy consumed by MCP and GraphQL. | Sections 4.2 and 6 still point part of that ownership back into `mcp-server`, leaving the reuse boundary unclear. | Auth architecture. |
| INT-02 | GraphQL auth/context seam | Proposal plus current repo | 2026-04-15 | High | The proposal now names both the transport-layer and resolver-layer owners required for GraphQL caller attribution. | This area is no longer a blocking readiness gap. | Closed stale blocker. |
| INT-03 | `command_journal` ownership | Stable refs plus current repo | 2026-04-15 | High | `command_journal` remains the canonical audit writer for mutating commands across northbound surfaces. | Risk 11.4 still reintroduces a stale server-owned redaction path. | Audit architecture. |
| INT-04 | test-gate identity | Stable refs plus current repo | 2026-04-15 | High | P029's proof lane now uses a non-colliding slug that matches current repo occupancy. | The prior gate-collision finding is stale. | Verification lane. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | NAV-01, NAV-02, NAV-03, DATA-01, DATA-02 | MCP HTTP, MCP stdio, GraphQL | Entry auth seams are now concretely named for all active northbound surfaces. |
| Happy path | Specified | DOC-01, NAV-04, MAP-06 | tools/list, tools/call, command journal | First-wave scope is clear and materially aligned to HEAD. |
| Unauthorized / rejection | Specified | MAP-01, MAP-02, MAP-03, DATA-01, DATA-02 | HTTP, stdio, GraphQL | Rejection semantics are now concrete enough to implement and test. |
| Capability-filtered list/call | Partial | NAV-04, DATA-05, INT-01 | MCP server, shared auth boundary | Policy behavior is clear, but the owner path is still inconsistent. |
| Audit write | Partial | MAP-06, MAP-07, DATA-03, DATA-04, INT-03 | command handler, command journal | Canonical storage owner is correct, but redaction and northbound audit-pointer details still need one more pass. |
| GraphQL coexistence | Specified | MAP-04, MAP-05, INT-02 | GraphQL server, MutationRoot | Stage A coexistence is now explicit and implementable. |
| Rollback / migration | Partial | DOC-01 | migration and rollback note | Rollback exists, but final audit contract alignment should happen before handoff. |
| Deferred later slices | Deferred intentionally | DOC-01 | N/A | Future tool/resource work is properly fenced out. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | None specified | auth, capability, caller-attribution landing | Single-step landing is still the intended posture | Rollback note exists in section 7 | 2026-04-15 | Medium | No new feature-flag blocker surfaced. Remaining issues are text-level contract alignment problems. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | `command_journal` caller metadata | security, coexistence visibility, auditability | every mutating northbound command | 2026-04-15 | High | Instrumentation owner is correct, but the proposal still needs one consistent redaction path and one explicit client-facing audit-pointer decision. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal proof lane | transport auth, capability filtering, caller attribution, GraphQL/MCP parity | Gate slug and inventory are now materially aligned to the repo | Audit-pointer tests need the northbound contract to be specified or removed | 2026-04-15 | High | The gate is plausible, but AC-11 is still underspecified. |
| TEST-02 | Existing repo protocol and handler seams | MCP/GraphQL command paths | Current code already exposes the owner seams the proposal targets | Shared-auth owner drift could create duplicate or ad hoc test seams unless the proposal picks one canonical boundary | 2026-04-15 | Medium | The remaining test risk is architectural consistency, not missing repo hooks. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | proof-lane identity | R3 uses `proposal-029-mcp` and leaves existing `proposal-029` untouched | `proposal-029|p029` is already occupied by the ACP second-wave runtime lane | 2026-04-15 | High | The old gate-collision blocker is closed and must be removed from the review. |
| REAL-02 | GraphQL principal attribution | R3 adds a middleware plus `ctx.data::<Principal>()?` path | Current GraphQL mount and resolvers are the exact seams that need this work | 2026-04-15 | High | The old "missing GraphQL auth seam" blocker is closed. |
| REAL-03 | stdio caller identity | R3 binds auth bootstrap to `initialize.params.clientInfo.principal_token` | Current stdio server is pure JSON-RPC and can extend `initialize` without inventing a second transport | 2026-04-15 | High | The old stdio-ambiguity blocker is closed. |
| REAL-04 | audit privacy contract | Section 4.3 says engine-owned `command_journal_redact.rs` | Risk 11.4 still says `mcp-server/src/audit.rs::redact(tool_name, args)` redacts before insert | 2026-04-15 | High | Proposal still contains a core owner-path contradiction on the audit path. |
| REAL-05 | shared auth/capability helpers | Section 4.1 creates a shared `control-plane/crates/auth` crate | Section 4.2 and section 6 still point to `mcp-server/src/auth.rs` for filtering | 2026-04-15 | High | The shared policy boundary is still ambiguous. |
| REAL-06 | client-visible audit pointer | AC-11 says callers can request `journal_id` | Proposal defines only the internal `Commanded` wrapper and no MCP or GraphQL response contract | 2026-04-15 | High | Proposal still requires one more contract decision before handoff. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | R3 has a clear delta posture. |
| Scope boundaries | Specified | DOC-01 | Deferred future tool/resource families are clearly fenced out. |
| Reusable baseline coverage | Specified | DOC-03, DOC-04, DOC-05 | Baseline alignment is materially improved. |
| Navigation / entry points | Specified | NAV-01, NAV-02, NAV-03, NAV-04, NAV-05 | All major northbound seams are now named concretely. |
| State handling | Partial | H matrix | Audit and capability-policy states still need one more alignment pass. |
| Data / API contract | Partial | DATA-03, DATA-04, DATA-05 | Audit pointer and auth-helper ownership are not fully locked down yet. |
| Persistence / caching | Partial | MAP-06, MAP-07, REAL-04 | `command_journal` ownership is correct, but the privacy text still conflicts with it. |
| Permissions / auth | Partial | DATA-01, DATA-02, DATA-05 | Runtime auth seams are now concrete, but helper ownership is still inconsistent. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Not a blocker on its own. |
| Analytics / instrumentation | Partial | METRIC-01 | Instrumentation owner is right, but audit-pointer scope needs one explicit decision. |
| Testing strategy | Partial | TEST-01, TEST-02 | Proof lane is much stronger, but one acceptance criterion is still under-specified. |
| Dependencies / integration points | Partial | INT-01, INT-03 | Shared auth boundary and audit path still need cleanup. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: R3 is authoritative over the stale local review artifacts.
- ASSUMP-02: `CommandHandler` remains the canonical mutating command and journal owner.
- ASSUMP-03: Stage B and Stage C work are future slices and not readiness blockers for P029 Stage A.
- QUESTION-01: Does P029 want to change MCP and GraphQL response schemas to expose `journal_id`, or should that promise be deferred?
- QUESTION-02: Is `control-plane/crates/auth` the canonical home for `filter_tools` and `filter_resources`, with transports only calling into it?
- BLOCKER-01: No `Critical` blocker remains on the current R3 draft. Remaining issues are a `High` audit-contract contradiction and a `Medium` auth-owner ambiguity that should be fixed before implementation handoff.

## O. Research Triggers / External Questions
Not used in this round. Local proposal, docs, baseline, and current code were sufficient for a defensible proposal-readiness judgment.
