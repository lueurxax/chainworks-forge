# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Overall readiness: `Red`
- Confidence: `High`
- Proposal completeness signal: `Mixed`
- Proposal reviewed: `docs/proposals/029-mcp-northbound-control-plane-server.md`
- Evidence pack: `docs/proposals/029-mcp-northbound-control-plane-server.review/evidence-pack.md`
- Reusable baseline used: `docs/reference/current-system-baseline.md`, `docs/reference/rust-control-plane.md`
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`
- Product overlay: `Not triggered`
- Current code areas inspected: `mcp-server/src/{server,http,protocol}.rs`, `mcp-server/src/tools/{mod,runs,ideas,steward}.rs`, `graphql-server/src/{server,auth_layer,schema}.rs`, `domain/src/commands.rs`, `auth/src/lib.rs`, `engine/src/command_handler.rs`, `db/src/repos/command_journal.rs`, `daemon/src/main.rs`, `.mcp.json`, `CLAUDE.md`, `scripts/test-gate.sh`, `docs/reference/test-gates.md`
- Remaining blockers: one Critical proposal-scope blocker around active Steward MCP/resource surface drift.

## 1. Executive Summary
P029 R6 closes the previous proposal blockers. Type ownership is now specified in the proposal, Stage A is narrowed to MCP command tools, the SwiftUI GraphQL-consumer claim is corrected, GraphQL WS unknown-token proof is added, and bootstrap token handling is hardened in the text.

The fresh blocker is current-repo drift: this working tree now has P049 Steward northbound surfaces. `mcp-server` registers `steward.run_analysis`, `steward.list_analyses`, `steward.get_analysis`, and `steward-analysis://{analysis_id}`. P029 still defines the "current" MCP tool/resource inventory without them and builds `CapabilityToolId`, `ResourceTemplateId`, class policy, ACs, and proof inventory from that stale list. If implemented as written on this tree, P029 would either hide/deny active Steward functionality or fail to provide the compile-time capability-drift guarantee it claims.

Readiness is `Red` only because of that active-surface omission. The older R5/R6 findings are closed at proposal-text level.

## 2. Proposal Scope and Completeness
- In scope: auth on MCP HTTP, MCP stdio, GraphQL HTTP, and GraphQL WS; capability filtering for tools/resources; caller-attributed `command_journal`; engine-owned redaction; `journal_id` surfacing; GraphQL coexistence; dogfood `.mcp.json` migration.
- Out of scope: new tool/resource expansion, southbound per-agent MCP policy, token rotation/revocation/delegation, UI rewrite, GraphQL mutation removal, MCP protocol bump for `structuredContent`.
- Most important closed stale issues: domain/auth/server type ownership is defined; direct vs command tool wording is narrowed; SwiftUI GraphQL migration claim is corrected; WS unknown-token test is present; bootstrap file mode and one-time token logging are specified.
- Most important current contradiction: active P049 Steward MCP tools/resources are now part of the current tree but absent from P029's capability/resource model.

## 3. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Amber | Medium | Complete | 0 | 0 | 1 | 0 |
| Architecture | Red | High | Complete | 1 | 1 | 0 | 0 |

## 4. Findings by Discipline

### 4.1 UI Findings
None. P029 has no proposal-owned visual surface and explicitly excludes a UI rewrite.

### 4.2 UX Findings
- Finding ID: `UX-029-01`
  Severity: `Medium`
  Evidence IDs: `DOC-06`, `NAV-07`, `FLAG-03`, `TEST-05`, `REAL-06`
  Why it matters:
  P029 correctly specifies same-commit `.mcp.json` and `CLAUDE.md` migration to `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`. Current repo files still use bare HTTP, so dogfooding will break if implementation lands without that config/doc change.
  Recommended fix:
  Keep this as an explicit implementation acceptance item. If the implementation owner cannot prove Claude Code header/env expansion locally, run the bounded external docs check listed as `RSH-01`.
  Acceptance criteria:
  - `.mcp.json` includes the `Authorization` header with `CHAINWORKS_MCP_TOKEN`.
  - `CLAUDE.md` documents how to read or re-bootstrap the token.
  - A focused dogfood or HTTP test proves an authorized request succeeds and a missing token fails.
  Confidence: `Medium`

### 4.3 Architecture Findings
- Finding ID: `ARCH-029-01`
  Severity: `Critical`
  Evidence IDs: `DOC-03`, `DOC-05`, `NAV-05`, `NAV-06`, `MAP-01`, `MAP-02`, `MAP-03`, `DATA-02`, `DATA-03`, `INT-01`, `REAL-01`, `REAL-02`, `REAL-03`, `TEST-02`
  Why it matters:
  P029 claims one `CapabilityToolId` variant per currently registered tool and one `ResourceTemplateId` variant per current resource template, then uses those closed enums as the compile-time drift guard. The current tree registers `steward.run_analysis`, `steward.list_analyses`, `steward.get_analysis`, and `steward-analysis://{analysis_id}`. P029 omits all of them from §2.1, §2.2, §4.0, §4.2, §6, AC-11/AC-13, and the `proposal-029-mcp` inventory. This undercuts the central auth/capability guarantee.
  Recommended fix:
  Update P029 to absorb the active Steward northbound surface or explicitly sequence P029 against a clean pre-P049 base. If absorbed, add `StewardRunAnalysis`, `StewardListAnalyses`, `StewardGetAnalysis` and `StewardAnalysisEntity`/equivalent resource template IDs, define class policy for each, classify `steward.run_analysis` as a command tool, and add focused auth/capability/journal/resource tests.
  Acceptance criteria:
  - §2.1 and §2.2 list the active Steward tools/resource or explicitly mark them unavailable on the implementation base.
  - `CapabilityToolId` and `ResourceTemplateId` include the active Steward variants if P049 remains in scope.
  - Class policy states whether `steward.run_analysis` is operator-only and which classes can read analysis results.
  - AC-11 includes `steward.run_analysis` as a command tool if active.
  - `proposal-029-mcp` includes `tools/list`, denied call, `resources/list`, `resources/read`, and `command_journal` assertions for Steward.
  Confidence: `High`

- Finding ID: `ARCH-029-02`
  Severity: `High`
  Evidence IDs: `MAP-04`, `MAP-05`, `MAP-06`, `INT-02`, `REAL-04`, `REAL-05`
  Why it matters:
  R6 proposal text now defines the right owner graph, but the current dirty implementation scaffold does not match it: `PrincipalClass` and string `ToolSpec` still live in `auth`, `CallerContext` stores principal class as a string, there is no `domain/src/capabilities.rs`, and daemon principal loading still uses local `principals.json`. This is not a proposal-text blocker if implementation starts from clean HEAD, but it is a handoff blocker on the current tree because implementers may patch around the old scaffold instead of replacing it with the R6 contract.
  Recommended fix:
  Add a short implementation note to P029 or the handoff plan: current partial auth scaffold must be replaced with the R6 owner graph, not incrementally preserved. Alternatively, clean the working tree before implementation so the proposal's "current baseline" statements are true again.
  Acceptance criteria:
  - Implementation handoff explicitly says the existing auth scaffold is stale relative to R6.
  - `domain` owns `PrincipalClass`, `CapabilityToolId`, and `ResourceTemplateId`.
  - `auth` depends on `domain` and consumes typed IDs, not string `ToolSpec`.
  - daemon principal loading uses the R6 env/default path contract and file-mode behavior.
  Confidence: `High`

## 5. Cross-Discipline Conflicts and Decisions
- Conflict: P029 wants compile-time capability drift mitigation, while current repo changed the MCP surface after the proposal's inventory was written.
  Tradeoff: Ignoring Steward keeps P029 narrower but breaks current-surface auth correctness; absorbing Steward increases scope but preserves the stated northbound-auth invariant.
  Decision: For this working tree, absorb Steward into P029 or declare an explicit sequencing base. Do not leave it implicit.
  Owner: Proposal author.

## 6. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Reconcile P029 with active P049 Steward MCP tools/resource | Architecture | Proposal author | Before implementation handoff | current P049 baseline decision | Capability/resource inventory covers every active MCP tool/resource on this tree | `ARCH-029-01` |
| P1 | Add implementation handoff note for replacing stale auth scaffold | Architecture | Proposal author / implementation owner | Before coding continues | P0 optional | Current dirty scaffold cannot be mistaken for R6-compliant implementation | `ARCH-029-02` |
| P2 | Preserve dogfood config/docs migration as a same-commit implementation gate | UX | Implementation owner | During implementation | auth principal bootstrap | Local MCP dogfood path works with token and fails without token | `UX-029-01` |

## 7. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Auth and capability policy | unauthenticated rejection, class-filtered tools/resources, denied calls for all active tools | focused HTTP/stdio/GraphQL/WS tests pass | no auth-disabled default path; no active tool omitted from policy | `proposal-029-mcp` gate | hold if any active MCP tool/resource lacks a capability ID |
| Audit trail | caller-attributed `command_journal` rows and redacted payloads | command rows have `caller_surface`, principal, and tool/mutation | direct tools do not fake `journal_id`; all command tools include it | `proposal-029-mcp` gate | hold if `steward.run_analysis` is active but unaudited |
| Coexistence | matching GraphQL/MCP command outcome for shared commands | cross-surface parity test passes | no GraphQL-to-MCP crate dependency | Stage A implementation review | hold if semantic divergence appears |
| Dogfood migration | authorized MCP HTTP path works with configured token env | `.mcp.json` + `CHAINWORKS_MCP_TOKEN` proof succeeds | token file owner-only permissions; no repeated token logging | implementation sign-off | hold if local MCP dogfood is stranded |

## 8. Evidence Gaps and Open Questions
### Evidence Gaps
- GAP-01: No external re-check was performed for Claude Code `.mcp.json` `headers` plus env expansion support. The proposal can remain locally reviewable, but implementation should prove it or re-check official docs if local proof fails.

### Open Questions
- QUESTION-01: Should `steward.run_analysis` be operator-only, or can `agent` class trigger manual Steward analyses?
- QUESTION-02: Should `steward.list_analyses` and `steward.get_analysis` be observer-readable, or limited to operator/agent because analysis artifacts can include operational details?
- QUESTION-03: Is P029 intended to land before P049 on a clean base, or after P049 on the current dirty tree?

## 9. Final Readiness Call
Readiness is `Red` on the current tree. R6 fixed the prior proposal-text issues, but P029 now misses active Steward MCP/resource surfaces introduced by P049/current baseline. Update the tool/resource inventory, capability IDs, class policy, ACs, and proof gate for Steward, or explicitly sequence P029 onto a pre-P049 base. After that, this proposal should likely move back to Green/Amber quickly because the old findings are closed.
